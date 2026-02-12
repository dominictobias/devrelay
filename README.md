# DevRelay

A local development reverse proxy with automatic HTTPS using Cloudflare Pingora.

## Features

- 🔀 Route custom domains to local development servers
- 🔒 Automatic HTTPS with self-signed certificates
- ⚡ Built on Cloudflare's high-performance [Pingora](https://github.com/cloudflare/pingora) framework
- 📝 Simple YAML configuration

## Quick Start

### 1. Configure Your Routes

Create a `config.yaml` file (or copy from `config.example.yaml`):

```yaml
# Proxy routes - each route maps a custom domain to a backend
routes:
  - host: "myapp.dev"
    port: 8080 # Port where proxy listens
    listen_tls: true # Required for .dev domains in Chrome
    backend: "localhost"
    backend_port: 3000 # Port where your dev server runs

  - host: "api.dev"
    port: 443 # HTTPS on standard port
    listen_tls: true # Accept HTTPS connections (default: false)
    backend: "localhost"
    backend_port: 5000

  - host: "frontend.dev"
    port: 443
    listen_tls: true # Accept HTTPS connections (default: false)
    backend: "localhost"
    backend_port: 8080
    backend_tls: false # Connect to backend over TLS (default: false)

# TLS/SSL Configuration
tls:
  enabled: true
  cert_dir: "./certs" # Directory to store generated certificates
  ca_name: "DevRelay CA" # Name for the Certificate Authority
```

### 2. Build and Run

```bash
cargo build --release
./target/release/devrelay
```

That's it! 🎉

On first run, DevRelay will **automatically**:

- ✅ Generate CA and server certificates
- ✅ Install the CA certificate to your macOS System Keychain (prompts for password)
- ✅ Add your custom domains to `/etc/hosts` (prompts for password)

Then just restart your browser and access `https://myapp.dev`!

## Usage

### Access Your Dev Servers

- `https://myapp.dev:8080` → proxies to `http://localhost:3000`
- `https://api.dev` → proxies to `http://localhost:5000`

### Custom Config Path

```bash
./target/release/devrelay --config config.example.yaml
```

### Skip Auto-Installation

If you want to install manually:

```bash
./target/release/devrelay --skip-install
```

### Force Reinstallation

To force reinstall the CA cert and hosts entries:

```bash
./target/release/devrelay --force-install
```

### Uninstall

To remove the CA certificate from your macOS Keychain and clean up `/etc/hosts` entries:

```bash
./target/release/devrelay --uninstall
```

## How It Works

1. **Routing**: Reads the `Host` header from incoming requests and matches it against configured routes
2. **Port Mapping**: Each route specifies both the listening port and backend port independently
3. **TLS**: Generates a local CA certificate and signs server certificates for each configured domain
4. **Proxy**: Uses Pingora's high-performance reverse proxy to forward requests to your local dev servers

## Requirements

- Rust 1.70+
- macOS (for certificate installation commands; Linux support coming soon)

## License

MIT
