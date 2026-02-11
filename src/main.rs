mod certs;
mod config;
mod install;
mod proxy;

use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use install::Installer;
use proxy::{get_listen_addresses, DevRelayProxy};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "devrelay")]
#[command(about = "Local development reverse proxy with automatic HTTPS", long_about = None)]
struct Args {
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
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve config path - if relative, look next to executable first
    let config_path = if args.config.is_absolute() {
        args.config
    } else {
        // Try next to executable first
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));

        if let Some(exe_dir) = exe_dir {
            let exe_config = exe_dir.join(&args.config);
            if exe_config.exists() {
                exe_config
            } else {
                args.config
            }
        } else {
            args.config
        }
    };

    println!("DevRelay - Local Development Proxy");
    println!("==================================\n");

    // Load configuration
    println!("Loading config from: {}", config_path.display());
    let config = Config::load(&config_path)
        .with_context(|| "Failed to load configuration")?;

    println!("Loaded {} route(s)\n", config.routes.len());

    // Handle uninstall
    if args.uninstall {
        let domains: Vec<String> = config.routes.iter().map(|r| r.host.clone()).collect();
        Installer::run_uninstall(&config.tls.ca_name, &domains)?;
        return Ok(());
    }

    // Initialize certificate manager and generate certificates
    let mut tls_cert_key: Option<(String, String)> = None;

    if config.tls.enabled {
        let cert_manager = certs::CertManager::new(&config.tls.cert_dir, config.tls.ca_name.clone());
        cert_manager.init()?;

        // Generate server certificates for all configured hosts
        for route in &config.routes {
            cert_manager.generate_server_cert(&route.host)?;
        }

        // Generate combined cert for TLS listeners (covers all listen_tls domains)
        let tls_domains: Vec<String> = config.routes
            .iter()
            .filter(|r| r.listen_tls)
            .map(|r| r.host.clone())
            .collect();

        if !tls_domains.is_empty() {
            cert_manager.generate_combined_server_cert(&tls_domains)?;
            tls_cert_key = Some((
                cert_manager.combined_cert_path().to_string_lossy().into_owned(),
                cert_manager.combined_key_path().to_string_lossy().into_owned(),
            ));
        }

        println!();

        // Auto-install CA cert and /etc/hosts entries if needed
        if !args.skip_install {
            let ca_cert_path = cert_manager.ca_cert_path();
            let domains: Vec<String> = config.routes.iter().map(|r| r.host.clone()).collect();

            let needs_install = args.force_install
                || !Installer::is_ca_installed(&ca_cert_path)?;

            if needs_install {
                Installer::run_install(&ca_cert_path, &domains)?;
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
            listen_proto, route.host, listen_port_str,
            backend_proto, route.backend, route.backend_port
        );
    }
    println!();

    // Build Pingora server
    let mut server = pingora_core::server::Server::new(None)
        .context("Failed to create server")?;
    server.bootstrap();

    let config_arc = Arc::new(config);
    let proxy = DevRelayProxy::new(config_arc.clone());

    let mut proxy_service = pingora_proxy::http_proxy_service(
        &server.configuration,
        proxy,
    );

    // Add listeners for all configured ports (TLS or TCP)
    let listen_addrs = get_listen_addresses(&config_arc);
    for listen_addr in &listen_addrs {
        if listen_addr.tls {
            if let Some((ref cert_path, ref key_path)) = tls_cert_key {
                proxy_service
                    .add_tls(&listen_addr.addr, cert_path, key_path)
                    .context(format!("Failed to add TLS listener on {}", listen_addr.addr))?;
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
    server.run_forever();
}
