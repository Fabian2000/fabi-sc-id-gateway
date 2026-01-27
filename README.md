# Fabi-SC ID Gateway

A lightweight authentication gateway for [Fabi-SC ID](https://id.fabi-sc.de). Protects upstream services with Fabi-SC ID access control and provides a web interface for configuration.

## Features

- **Reverse Proxy** - Route HTTP and WebSocket traffic to upstream services
- **Authentication** - Protect routes with Fabi-SC ID integration
- **Host-based Routing** - Route requests based on hostname
- **Web UI** - Configure routes and settings through a browser interface
- **SQLite Database** - No external database required
- **TLS Support** - Optional HTTPS with rustls

## Requirements

- Rust (for building)
- [grass](https://github.com/connorskees/grass) (for SCSS compilation, install via `cargo install grass`)

## Building

Use the provided build script:

```bash
./build.sh
```

This will:

1. Compile SCSS to CSS
2. Build the Rust binary in release mode

For manual builds:

```bash
# Compile SCSS
grass static/scss/main.scss static/css/main.css --style compressed

# Build Rust
cargo build --release
```

The binary will be at `target/release/fabi-sc-id-gateway`.

## Usage

```bash
# Start with default settings
fabi-sc-id-gateway

# Specify port and host
fabi-sc-id-gateway --port 3000 --host 127.0.0.1

# Enable TLS
fabi-sc-id-gateway --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem

# Custom database location
fabi-sc-id-gateway --database /var/lib/gateway/data.db
```

### Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --port` | Port to listen on | `8080` |
| `--host` | Host to bind to | `0.0.0.0` |
| `-d, --database` | SQLite database path | `gateway.db` |
| `--tls-cert` | TLS certificate (PEM) | - |
| `--tls-key` | TLS private key (PEM) | - |
| `--log-level` | Log level | `info` |

## Configuration

On first run, navigate to `/_admin/setup` to configure the Fabi-SC ID integration:

1. **Application ID** - Your application ID from Fabi-SC ID
2. **API Key** - Your application API key
3. **Gateway URL** - The public URL of this gateway (e.g., `https://gateway.example.com`)
4. **Admin Users** - Comma-separated Fabi-SC ID usernames

After setup, access the admin panel at `/_admin` to manage routes.

## Routes

Routes are configured based on hostname:

- **Name** - A display name for the route
- **Host** - The hostname to match (e.g., `app.example.com`)
- **Upstream URL** - The target server URL
- **Requires Auth** - Whether authentication is required
- **Allowed Users** - Optional whitelist of usernames (only applies when auth is required)

## Development

```bash
# Run in development mode
cargo run -- --log-level debug

# Compile SCSS
grass static/scss/main.scss static/css/main.css
```

## Systemd Service

Example service file (`/etc/systemd/system/fabi-sc-id-gateway.service`):

```ini
[Unit]
Description=Fabi-SC ID Gateway
After=network.target

[Service]
Type=simple
User=gateway
WorkingDirectory=/opt/fabi-sc-id-gateway
ExecStart=/opt/fabi-sc-id-gateway/fabi-sc-id-gateway --port 8080
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

## License

MIT
