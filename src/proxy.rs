use crate::config::{Config, Route};
use async_trait::async_trait;
use pingora_core::upstreams::peer::HttpPeer;
use pingora_proxy::{ProxyHttp, Session};
use std::sync::Arc;

pub struct DevRelayProxy {
    config: Arc<Config>,
}

impl DevRelayProxy {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    fn get_backend_for_host(&self, host: &str) -> Option<&Route> {
        self.config.get_route_by_host(host)
    }
}

#[async_trait]
impl ProxyHttp for DevRelayProxy {
    type CTX = ();

    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora_core::Result<Box<HttpPeer>> {
        // Get the Host header to determine routing
        let host = session
            .req_header()
            .headers
            .get("Host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        // Find the matching route
        let route = self
            .get_backend_for_host(host)
            .ok_or_else(|| {
                pingora_core::Error::explain(
                    pingora_core::ErrorType::HTTPStatus(404),
                    format!("No route configured for host: {}", host),
                )
            })?;

        // Create peer for the backend
        let peer = Box::new(HttpPeer::new(
            (route.backend.as_str(), route.backend_port),
            route.backend_tls,
            route.backend.clone(),
        ));

        let path = session.req_header().uri.path();
        println!(
            "Proxying {}{} -> {}:{}",
            host, path, route.backend, route.backend_port
        );

        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        _upstream_request: &mut pingora::http::RequestHeader,
        _ctx: &mut Self::CTX,
    ) -> pingora_core::Result<()> {
        // Forward the original Host header to the backend
        // This is useful if the backend needs to know the original host
        Ok(())
    }

    async fn fail_to_proxy(
        &self,
        _session: &mut Session,
        error: &pingora_core::Error,
        _ctx: &mut Self::CTX,
    ) -> pingora_proxy::FailToProxy {
        eprintln!("Failed to proxy request: {}", error);

        // Return appropriate error code
        let error_code = if error.etype() == &pingora_core::ErrorType::ConnectTimedout
            || error.etype() == &pingora_core::ErrorType::ConnectError
        {
            502 // Bad Gateway
        } else {
            500 // Internal Server Error
        };

        pingora_proxy::FailToProxy {
            error_code,
            can_reuse_downstream: true,
        }
    }
}

pub struct ListenAddr {
    pub addr: String,
    pub tls: bool,
}

pub fn get_listen_addresses(config: &Config) -> Vec<ListenAddr> {
    // Collect unique ports and whether they need TLS
    // If any route on a port has listen_tls, the whole port is TLS
    let mut port_tls: std::collections::HashMap<u16, bool> = std::collections::HashMap::new();
    for route in &config.routes {
        let entry = port_tls.entry(route.port).or_insert(false);
        if route.listen_tls {
            *entry = true;
        }
    }

    let mut result: Vec<ListenAddr> = port_tls
        .into_iter()
        .map(|(port, tls)| {
            let proto = if tls { "https" } else { "http" };
            let addr = format!("0.0.0.0:{}", port);
            println!("Listening on: {} ({})", addr, proto);
            ListenAddr { addr, tls }
        })
        .collect();
    result.sort_by(|a, b| a.addr.cmp(&b.addr));
    result
}
