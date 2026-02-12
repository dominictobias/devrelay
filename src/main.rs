mod certs;
mod config;
mod install;
mod proxy;

use anyhow::{Context, Result};
use certs::CertManager;
use clap::Parser;
use config::Config;
use install::Installer;
use proxy::{DevRelayProxy, get_listen_addresses};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "devrelay")]
#[command(about = "Local development reverse proxy with automatic HTTPS", long_about = None)]
enum Command {
    /// Start the proxy server (default)
    #[command(name = "start")]
    Server {
        /// Path to configuration file
        #[arg(short, long, default_value = "config.yaml")]
        config: PathBuf,

        /// Skip automatic installation of CA cert and /etc/hosts entries
        #[arg(long)]
        skip_install: bool,

        /// Force reinstallation even if already installed
        #[arg(long)]
        force_install: bool,

        /// Uninstall CA certificate and /etc/hosts entries, then exit
        #[arg(long)]
        uninstall: bool,

        /// Suppress per-request "Proxying ... -> ..." log lines
        #[arg(short, long)]
        quiet: bool,
    },
}

fn main() -> Result<()> {
    let command = Command::parse();

    match command {
        Command::Server {
            config,
            skip_install,
            force_install,
            uninstall,
            quiet,
        } => run_server(config, skip_install, force_install, uninstall, quiet)?,
    }

    Ok(())
}

fn run_server(
    config_arg: PathBuf,
    skip_install: bool,
    force_install: bool,
    uninstall: bool,
    quiet: bool,
) -> Result<()> {
    // Resolve config path - prefer current working directory (project root) for bare filenames
    let config_path = if config_arg.is_absolute() {
        config_arg
    } else if config_arg.starts_with(".") || config_arg.starts_with("..") {
        // Explicitly relative path (./foo or ../foo) - resolve from CWD
        config_arg
    } else {
        // Bare filename (e.g. config.yaml) - resolve from CWD so it works when run
        // from project root via npm/bun (bun run proxy server)
        std::env::current_dir()
            .map(|cwd| cwd.join(&config_arg))
            .unwrap_or(config_arg)
    };

    println!("DevRelay - Local Development Proxy");
    println!("==================================\n");

    // Load configuration
    println!("Loading config from: {}", config_path.display());
    let config = Config::load(&config_path).with_context(|| "Failed to load configuration")?;

    println!("Loaded {} route(s)\n", config.routes.len());

    // Handle uninstall
    if uninstall {
        let domains: Vec<String> = config.routes.iter().map(|r| r.host.clone()).collect();
        Installer::run_uninstall(&config.tls.ca_name, &domains)?;
        return Ok(());
    }

    // Initialize certificate manager and generate certificates
    let mut tls_cert_key: Option<(String, String)> = None;

    if config.tls.enabled {
        let cert_manager = CertManager::new(&config.tls.cert_dir, config.tls.ca_name.clone());
        cert_manager.init()?;

        // Generate server certificates for all configured hosts
        for route in &config.routes {
            cert_manager.generate_server_cert(&route.host)?;
        }

        // Generate combined cert for TLS listeners (covers all listen_tls domains)
        let tls_domains: Vec<String> = config
            .routes
            .iter()
            .filter(|r| r.listen_tls)
            .map(|r| r.host.clone())
            .collect();

        if !tls_domains.is_empty() {
            cert_manager.generate_combined_server_cert(&tls_domains)?;
            tls_cert_key = Some((
                cert_manager
                    .combined_cert_path()
                    .to_string_lossy()
                    .into_owned(),
                cert_manager
                    .combined_key_path()
                    .to_string_lossy()
                    .into_owned(),
            ));
        }

        println!();

        // Auto-install CA cert and /etc/hosts entries if needed
        if !skip_install {
            let ca_cert_path = cert_manager.ca_cert_path();
            let ca_name = &config.tls.ca_name;
            let domains: Vec<String> = config.routes.iter().map(|r| r.host.clone()).collect();

            let needs_install =
                force_install || !Installer::is_ca_installed(&ca_cert_path, ca_name)?;

            if needs_install {
                Installer::run_install(&ca_cert_path, ca_name, &domains)?;
            } else {
                // Still check hosts entries even if CA is installed
                println!("✅ CA certificate already installed\n");
                println!("🌐 Checking /etc/hosts entries...");
                Installer::install_hosts_entries(&domains)?;
            }

            println!();
        }
    }

    // Print route information
    println!("Configured routes:");
    for route in &config.routes {
        let listen_proto = if route.listen_tls { "https" } else { "http" };
        let backend_proto = if route.backend_tls { "https" } else { "http" };
        let listen_default_port = if route.listen_tls { 443 } else { 80 };
        let listen_port_str = if route.port == listen_default_port {
            String::new()
        } else {
            format!(":{}", route.port)
        };
        println!(
            "  {}://{}{} -> {}://{}:{}",
            listen_proto,
            route.host,
            listen_port_str,
            backend_proto,
            route.backend,
            route.backend_port
        );
    }
    println!();

    // Build Pingora server
    let mut server = pingora_core::server::Server::new(None).context("Failed to create server")?;
    server.bootstrap();

    let config_arc = Arc::new(config);
    let proxy = DevRelayProxy::new(config_arc.clone(), quiet);

    let mut proxy_service = pingora_proxy::http_proxy_service(&server.configuration, proxy);

    // Add listeners for all configured ports (TLS or TCP)
    let listen_addrs = get_listen_addresses(&config_arc);

    // Pre-check that ports are not already in use (Pingora would panic on bind otherwise)
    for listen_addr in &listen_addrs {
        if let Err(e) = std::net::TcpListener::bind(&listen_addr.addr) {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                let port = listen_addr.addr.rsplit(':').next().unwrap_or("?");
                anyhow::bail!(
                    "Port already in use: {}. Stop the process using it or use a different port in your config (e.g. config.yaml).\n\n  To find and stop the process:\n    1. lsof -i :{}\n    2. kill <PID>   (use the PID from the output, e.g. kill 650)",
                    listen_addr.addr,
                    port
                );
            }
            return Err(e).context(format!("Cannot bind to {}", listen_addr.addr));
        }
    }

    for listen_addr in &listen_addrs {
        if listen_addr.tls {
            if let Some((ref cert_path, ref key_path)) = tls_cert_key {
                proxy_service
                    .add_tls(&listen_addr.addr, cert_path, key_path)
                    .context(format!(
                        "Failed to add TLS listener on {}",
                        listen_addr.addr
                    ))?;
            } else {
                eprintln!(
                    "Warning: route on {} has listen_tls but TLS is not enabled in config, falling back to TCP",
                    listen_addr.addr
                );
                proxy_service.add_tcp(&listen_addr.addr);
            }
        } else {
            proxy_service.add_tcp(&listen_addr.addr);
        }
    }

    server.add_service(proxy_service);

    println!("Starting DevRelay proxy...\n");
    // Shutdown: Ctrl+C (SIGINT) = fast exit; kill <PID> or SIGTERM = graceful shutdown.
    server.run_forever();
}
