use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub routes: Vec<Route>,
    pub tls: TlsConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Route {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub listen_tls: bool,
    pub backend: String,
    pub backend_port: u16,
    #[serde(default)]
    pub backend_tls: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_dir: String,
    pub ca_name: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse YAML config")?;

        Ok(config)
    }

    pub fn get_route_by_host(&self, host: &str) -> Option<&Route> {
        // Strip port from host if present (e.g., "myapp.dev:8080" -> "myapp.dev")
        let host_without_port = host.split(':').next().unwrap_or(host);
        self.routes.iter().find(|r| r.host == host_without_port)
    }
}
