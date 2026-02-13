# DevRelay

A local development reverse proxy with automatic HTTPS using Cloudflare Pingora.

## Features

- 🔀 Route custom domains to local development servers
- 🔒 Automatic HTTPS with self-signed certificates
- ⚡ Built on Cloudflare's high-performance [Pingora](https://github.com/cloudflare/pingora) framework
- 📝 Simple YAML configuration

## Install

```bash
npm install -g @devrelay/cli
bun install -g @devrelay/cli
pnpm add -g @devrelay/cli
yarn global add @devrelay/cli
```

Or run without installing:

```bash
npx @devrelay/cli
bunx @devrelay/cli
pnpx @devrelay/cli
yarn dlx @devrelay/cli
```

**Supported platforms:** macOS (arm64, x64) and Linux (arm64, x64)

## Quick Start

### 1. Configure routes

Create a `config.yaml` in your project:

```yaml
routes:
  - host: "myapp.dev"
    port: 8080
    listen_tls: true
    backend: "localhost"
    backend_port: 3000
    backend_tls: false # true for https://localhost (default = false)

tls:
  enabled: true
  cert_dir: "./certs"
  ca_name: "DevRelay CA"
```

### 2. Run

```bash
devrelay start
```

On first run, DevRelay will automatically generate certificates, install its CA to your system trust store, and add your domains to `/etc/hosts`. Restart your browser and visit `https://myapp.dev`. If you had a server running on `http://localhost:3000` it will be loaded.

## Usage

| Command                                       | Description                                   |
| --------------------------------------------- | --------------------------------------------- |
| `devrelay start`                              | Start with default `config.yaml`              |
| `devrelay start --config path/to/config.yaml` | Use a custom config file                      |
| `devrelay start --skip-install`               | Skip CA cert and hosts setup (manual install) |
| `devrelay start --force-install`              | Reinstall CA cert and hosts entries           |
| `devrelay start --quiet`                      | Suppress per-request proxying log lines       |
| `devrelay start --uninstall`                  | Remove CA cert and hosts entries              |

## Configuration

Each route maps a hostname to a backend:

| Field          | Description                                  |
| -------------- | -------------------------------------------- |
| `host`         | Domain to listen for (e.g. `myapp.dev`)      |
| `port`         | Port the proxy listens on                    |
| `listen_tls`   | Accept HTTPS (required for `.dev` in Chrome) |
| `backend`      | Backend host (usually `localhost`)           |
| `backend_port` | Port your dev server runs on                 |
| `backend_tls`  | Connect to backend over HTTPS                |

## License

MIT
