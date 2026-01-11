# tgcp

**Terminal UI for Google Cloud Platform**

[![CI](https://github.com/MrMichou/tgcp/actions/workflows/ci.yml/badge.svg)](https://github.com/MrMichou/tgcp/actions/workflows/ci.yml)
[![Security](https://github.com/MrMichou/tgcp/actions/workflows/security.yml/badge.svg)](https://github.com/MrMichou/tgcp/actions/workflows/security.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org/)

A fast, keyboard-driven terminal interface for navigating and managing Google Cloud Platform resources. Inspired by vim, built with Rust.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  tgcp                          my-project  │  us-central1-a  │  instances  │
├─────────────────────────────────────────────────────────────────────────────┤
│  NAME                 STATUS     MACHINE TYPE     ZONE            IP        │
│  ─────────────────────────────────────────────────────────────────────────  │
│▸ web-server-01        RUNNING    e2-medium        us-central1-a   10.0.0.2  │
│  web-server-02        RUNNING    e2-medium        us-central1-a   10.0.0.3  │
│  database-primary     RUNNING    n2-standard-4    us-central1-a   10.0.0.4  │
│  batch-worker-01      STOPPED    e2-small         us-central1-a   -         │
│  dev-instance         RUNNING    e2-micro         us-central1-a   10.0.0.6  │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  j/k:navigate  d:describe  s:start  S:stop  p:project  z:zone  ?:help  q:quit│
└─────────────────────────────────────────────────────────────────────────────┘
```

## Features

- **Vim-style navigation** - `j/k`, `gg`, `G`, `/` search, `:` commands
- **Multi-resource support** - VMs, disks, networks, buckets, GKE clusters
- **Hierarchical browsing** - Navigate from VMs to disks, buckets to objects
- **Resource actions** - Start, stop, reset, delete with confirmation dialogs
- **Real-time filtering** - Instant search across resource lists
- **Project/zone switching** - Quick context changes without leaving the app
- **Read-only mode** - Safe exploration with `--readonly` flag
- **All-zones view** - See resources across all zones at once
- **JSON detail view** - Full resource inspection with `d` key
- **Async & fast** - Non-blocking API calls with pagination support

## Installation

### From source

```bash
# Clone the repository
git clone https://github.com/MrMichou/tgcp.git
cd tgcp

# Build release binary
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .
```

### Prerequisites

- Rust stable (latest recommended)
- Google Cloud SDK (`gcloud`) configured with credentials

## Quick Start

1. **Authenticate with GCP**
   ```bash
   gcloud auth application-default login
   ```

2. **Set default project and zone** (optional)
   ```bash
   gcloud config set project my-project
   gcloud config set compute/zone us-central1-a
   ```

3. **Launch tgcp**
   ```bash
   tgcp
   ```

   Or with explicit project/zone:
   ```bash
   tgcp --project my-project --zone us-central1-a
   ```

## Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `gg` | Go to first item |
| `G` | Go to last item |
| `[` / `]` | Previous / next page |
| `b` / `Backspace` | Go back |

### Actions

| Key | Action |
|-----|--------|
| `Enter` / `d` | View resource details (JSON) |
| `s` | Start instance |
| `S` | Stop instance |
| `r` | Reset instance |
| `Ctrl+d` | Delete resource (with confirmation) |
| `R` | Refresh current view |

### Context

| Key | Action |
|-----|--------|
| `p` | Switch project |
| `z` | Switch zone |
| `/` | Filter resources |
| `:` | Command mode |
| `?` | Show help |
| `q` | Quit |

### Command Mode (`:`)

| Command | Action |
|---------|--------|
| `:instances` | Go to VM instances |
| `:disks` | Go to persistent disks |
| `:buckets` | Go to Cloud Storage |
| `:clusters` | Go to GKE clusters |
| `:zone us-west1-a` | Switch zone |
| `:project my-proj` | Switch project |
| `:q` | Quit |

## Supported Resources

### Compute Engine
- **VM Instances** - View, start, stop, reset, delete
- **Persistent Disks** - View, delete
- **VPC Networks** - View
- **Subnets** - View
- **Firewall Rules** - View, delete

### Cloud Storage
- **Buckets** - View, navigate to objects
- **Objects** - View, download URL

### Google Kubernetes Engine
- **Clusters** - View, navigate to node pools
- **Node Pools** - View

### Cloud CDN / Load Balancing
- **Backend Services** - View, delete (shows CDN status)
- **Backend Buckets** - View, delete (static content CDN)
- **URL Maps** - View, delete (routing rules)
- **HTTP/HTTPS Proxies** - View, delete
- **TCP/SSL/gRPC Proxies** - View, delete
- **Forwarding Rules** - View, delete (external IPs)
- **SSL Certificates** - View, delete (managed & self-managed)
- **Health Checks** - View, delete
- **Target Pools** - View, delete (legacy network LB)
- **SSL Policies** - View, delete (TLS configuration)
- **Security Policies** - View, delete (Cloud Armor WAF/DDoS)
- **Network Endpoint Groups** - View, delete (NEGs)

## Configuration

Configuration is stored at `~/.config/tgcp/config.json`:

```json
{
  "project_id": "my-project",
  "zone": "us-central1-a",
  "last_resource": "compute-instances"
}
```

### Command-line Options

```bash
tgcp [OPTIONS]

Options:
  -p, --project <PROJECT>    GCP project ID
  -z, --zone <ZONE>          Compute zone
  -r, --readonly             Read-only mode (disable actions)
  -l, --log-level <LEVEL>    Log level [default: info]
  -h, --help                 Print help
  -V, --version              Print version
```

### Logging

Logs are written to `~/.config/tgcp/tgcp.log`. Enable debug logging:

```bash
tgcp --log-level debug
```

## Authentication

tgcp uses [Application Default Credentials (ADC)](https://cloud.google.com/docs/authentication/application-default-credentials):

```bash
# User credentials (recommended for local development)
gcloud auth application-default login

# Or use a service account
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/key.json
```

## Troubleshooting

### "Permission denied" errors
Ensure your account has the necessary IAM permissions for the resources you're accessing (e.g., `compute.instances.list`).

### "Authentication failed"
Refresh your credentials:
```bash
gcloud auth application-default login
```

### Terminal rendering issues
- Ensure your terminal supports 256 colors
- Try resizing your terminal window
- Check that UTF-8 encoding is enabled

## Development

See [CLAUDE.md](CLAUDE.md) for development documentation, architecture details, and contribution guidelines.

```bash
# Run in development
cargo run

# Run tests
cargo test

# Run lints
cargo clippy

# Format code
cargo fmt
```

## Tech Stack

- **[Rust](https://www.rust-lang.org/)** - Systems programming language
- **[ratatui](https://ratatui.rs/)** - Terminal UI framework
- **[tokio](https://tokio.rs/)** - Async runtime
- **[reqwest](https://docs.rs/reqwest)** - HTTP client with rustls
- **[gcp_auth](https://docs.rs/gcp_auth)** - GCP authentication

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Based on [taws](https://github.com/huseyinbabal/taws) - Terminal UI for AWS
- Inspired by [k9s](https://k9scli.io/) for Kubernetes
- Built with the excellent [ratatui](https://ratatui.rs/) TUI framework
