# CLAUDE.md - tgcp Development Guide

> AI Assistant Development Guide for tgcp (Terminal UI for GCP)

---

## Table of Contents

- [Project Overview](#project-overview)
- [Tech Stack](#tech-stack)
- [Architecture](#architecture)
- [Getting Started](#getting-started)
- [Usage](#usage)
- [Development](#development)
- [Testing](#testing)
- [Contributing](#contributing)
- [Release Process](#release-process)
- [Troubleshooting](#troubleshooting)

---

## Project Overview

**tgcp** (Terminal UI for GCP) is a Rust-based terminal user interface for navigating, observing, and managing Google Cloud Platform resources. It provides a vim-style keyboard-driven experience for interacting with GCP services directly from the terminal.

### Key Features

- **Vim-style navigation** with keyboard shortcuts
- **Multi-resource support** for Compute, Storage, GKE, Load Balancing
- **Data-driven architecture** with JSON resource definitions
- **Theme system** with per-project themes
- **SSH integration** with IAP tunnel support
- **Hierarchical navigation** with sub-resources and breadcrumbs

## Tech Stack

- **Language**: Rust (2021 edition)
- **TUI Framework**: ratatui 0.30 + crossterm 0.29
- **Async Runtime**: tokio (full features)
- **HTTP Client**: reqwest with rustls-tls (avoids OpenSSL cross-compilation issues)
- **GCP Auth**: gcp_auth 0.12
- **Serialization**: serde + serde_json
- **CLI**: clap 4.5 with derive feature
- **Logging**: tracing + tracing-subscriber + tracing-appender
- **Error Handling**: anyhow + thiserror

## Architecture

### Module Structure

```
src/
├── main.rs           # Entry point, terminal setup, splash screen
├── app.rs            # Central application state (App struct)
├── config.rs         # Persistent configuration management
├── event.rs          # Keyboard/event handling
├── gcp/              # GCP API interaction
│   ├── mod.rs
│   ├── auth.rs       # Authentication (gcloud credentials)
│   ├── client.rs     # GcpClient - main API client
│   ├── http.rs       # HTTP utilities for REST calls
│   └── projects.rs   # Project management
├── resource/         # Resource abstraction layer
│   ├── mod.rs
│   ├── registry.rs   # JSON resource definitions loader
│   ├── fetcher.rs    # Resource fetching with pagination
│   └── sdk_dispatch.rs # Maps SDK methods to REST API calls
├── resources/        # JSON resource definitions (compiled into binary)
│   ├── common.json   # Shared color maps
│   ├── compute.json  # Compute Engine resources
│   ├── storage.json  # Cloud Storage resources
│   └── gke.json      # GKE resources
├── shell/            # Shell integration (SSH, exec)
│   └── mod.rs        # SSH, serial console, browser launch
├── theme/            # Theme system
│   └── mod.rs        # Theme definitions and manager
└── ui/               # UI rendering
    ├── mod.rs        # Main render function
    ├── splash.rs     # Startup splash screen
    ├── header.rs     # Header bar with project/zone info
    ├── help.rs       # Help overlay (? key)
    ├── dialog.rs     # Confirmation dialogs
    ├── command_box.rs # Command mode (: key)
    ├── projects.rs   # Project selector
    └── zones.rs      # Zone selector
```

### Key Design Patterns

1. **Data-Driven Resource Definitions**: Resources are defined in JSON files (`src/resources/*.json`) and compiled into the binary. This allows adding new GCP resource types without code changes.

2. **SDK Dispatch Pattern**: `sdk_dispatch.rs` maps abstract method names (e.g., `list_instances`) to concrete REST API calls. This decouples resource definitions from API implementation details.

3. **Mode-Based UI**: The app uses distinct modes (Normal, Command, Help, Confirm, Describe, Projects, Zones) with mode-specific event handling.

4. **Hierarchical Navigation**: Resources can have sub-resources (e.g., VM → Disks, Bucket → Objects) with breadcrumb navigation.

5. **Async Everything**: All GCP API calls are async using tokio runtime.

---

## Getting Started

### Prerequisites

- **Rust**: 1.70+ (2021 edition)
- **gcloud CLI**: Authenticated and configured
- **GCP Project**: With appropriate IAM permissions

### Authentication

The app uses Application Default Credentials (ADC). Ensure you're authenticated:

```bash
gcloud auth application-default login
```

The app reads default project/zone from gcloud config:

```bash
gcloud config get-value project
gcloud config get-value compute/zone
```

### Installation

#### From Source

```bash
git clone https://github.com/mnicolet/tgcp.git
cd tgcp
cargo build --release
./target/release/tgcp
```

#### From GitHub Releases

Download pre-built binaries from the [releases page](https://github.com/mnicolet/tgcp/releases):

```bash
# Linux x86_64
curl -L https://github.com/mnicolet/tgcp/releases/latest/download/tgcp-linux-x86_64.tar.gz | tar xz

# macOS (Apple Silicon)
curl -L https://github.com/mnicolet/tgcp/releases/latest/download/tgcp-darwin-aarch64.tar.gz | tar xz

# Move to PATH
sudo mv tgcp /usr/local/bin/
```

---

## Usage

### Build & Run

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run with default project from gcloud config
cargo run

# Run with specific project and zone
cargo run -- --project my-project --zone us-central1-a

# Run in read-only mode (no write operations)
cargo run -- --readonly

# Enable debug logging
cargo run -- --log-level debug
```

## Key Commands

| Key | Action |
|-----|--------|
| `j/k` or `↑/↓` | Navigate up/down |
| `gg` | Go to top |
| `G` | Go to bottom |
| `Enter` or `d` | View details (JSON) |
| `/` | Start filtering |
| `:` | Command mode |
| `?` | Help overlay |
| `p` | Switch project |
| `z` | Switch zone |
| `R` | Refresh |
| `[/]` | Previous/next page |
| `b` or `Backspace` | Go back |
| `q` | Quit |

### Resource Actions

| Key | Action |
|-----|--------|
| `s` | Start instance |
| `S` | Stop instance |
| `Ctrl+r` | Reset instance |
| `x` | SSH to instance |
| `X` | SSH via IAP tunnel |
| `C` | Open in GCP Console |
| `Ctrl+d` | Delete (destructive) |

### Commands (`:` mode)

| Command | Action |
|---------|--------|
| `:theme <name>` | Switch theme (dracula, monokai, nord, gruvbox, solarized, production) |
| `:alias <alias> <resource>` | Create command alias |
| `:<resource>` | Navigate to resource type |
| `:projects` | Open project selector |
| `:zones` | Open zone selector |

## Adding New GCP Resources

1. **Add resource definition** in `src/resources/<service>.json`:
```json
{
  "resources": {
    "service-resource": {
      "display_name": "Resource Name",
      "service": "servicename",
      "sdk_method": "list_resources",
      "response_path": "items",
      "id_field": "id",
      "name_field": "name",
      "is_global": false,
      "columns": [
        { "header": "NAME", "json_path": "name", "width": 25 }
      ],
      "sub_resources": [],
      "actions": []
    }
  }
}
```

2. **Implement SDK method** in `src/resource/sdk_dispatch.rs`:
```rust
async fn invoke_servicename(method: &str, client: &GcpClient, params: &Value) -> Result<Value> {
    match method {
        "list_resources" => {
            let url = client.service_url("resources");
            client.get(&url).await
        }
        _ => Err(anyhow::anyhow!("Unknown method: {}", method)),
    }
}
```

3. **Add URL helper** (if needed) in `src/gcp/client.rs`

## Configuration

Configuration is stored at:
- Linux/macOS: `~/.config/tgcp/config.json`
- Fallback: `~/.tgcp/config.json`

Logs are stored at:
- Linux/macOS: `~/.config/tgcp/tgcp.log`

### Config File Format
```json
{
  "project_id": "my-project",
  "zone": "us-central1-a",
  "theme": "dracula",
  "project_themes": {
    "production-project": "production",
    "dev-project": "default"
  },
  "aliases": {
    "vm": "compute-instances",
    "disk": "compute-disks",
    "fw": "compute-firewalls"
  },
  "ssh": {
    "use_iap": false,
    "extra_args": ["-o", "StrictHostKeyChecking=no"]
  }
}
```

## Themes

tgcp supports customizable themes, including per-project themes (useful for distinguishing production from dev environments).

### Built-in Themes
- `default` - Standard dark theme
- `dracula` - Popular dark theme with purple accents
- `monokai` - Classic code editor theme
- `nord` - Arctic color palette
- `gruvbox` - Retro groove theme
- `solarized` - Solarized dark
- `production` - Red-tinted theme to warn about production environments

### Switching Themes
```
# Via command mode
:theme dracula

# Via environment variable
TGCP_THEME=monokai tgcp
```

### Custom Themes
Place custom theme files in `~/.config/tgcp/skins/<name>.yaml`:
```yaml
name: my-theme
base:
  background: [30, 30, 30]
  foreground: [220, 220, 220]
  accent: [100, 200, 255]
table:
  header: [255, 200, 100]
  selected_bg: [60, 60, 60]
status:
  running: [100, 255, 100]
  stopped: [128, 128, 128]
```

### Per-Project Themes
Configure different themes for different projects in `config.json`:
```json
{
  "project_themes": {
    "my-prod-project": "production",
    "my-dev-project": "dracula"
  }
}
```
When switching to a project, its theme is automatically applied.

## SSH Integration

tgcp can SSH directly into VM instances without leaving the TUI.

### SSH Keys
- `x` - SSH to selected instance (uses `gcloud compute ssh`)
- `X` - SSH via IAP tunnel (for instances without external IP)
- `C` - Open instance in GCP Console browser

### SSH Configuration
In `~/.config/tgcp/config.json`:
```json
{
  "ssh": {
    "use_iap": true,
    "extra_args": ["-o", "StrictHostKeyChecking=no"]
  }
}
```

### How it works
1. tgcp suspends the TUI
2. Runs `gcloud compute ssh <instance> --zone <zone> --project <project>`
3. User interacts with SSH session
4. When SSH exits, tgcp resumes

## Code Conventions

### Rust Style
- Use `anyhow::Result` for error handling in application code
- Use `thiserror` for library-style error types
- Prefer `Option::map`/`and_then` over `if let` for transformations
- Use `tracing` macros (`tracing::info!`, `tracing::debug!`) for logging

### UI Conventions
- All tables have left-padded cells (` {value}`)
- Status values use color maps defined in `common.json`
- Transitional states (PENDING, STARTING, etc.) get a `↻` indicator
- Error messages are displayed in red in the footer

### Resource Definitions
- Use snake_case for `sdk_method` names
- Use kebab-case for resource keys
- Computed fields use `_short`, `_display`, or `_count` suffixes

---

## Development

### Development Workflow

1. **Clone the repository**

```bash
git clone https://github.com/mnicolet/tgcp.git
cd tgcp
```

2. **Create a feature branch**

```bash
git checkout -b feature/my-feature
```

3. **Make changes and test**

```bash
cargo test
cargo run
```

4. **Format and lint**

```bash
cargo fmt
cargo clippy
```

5. **Commit and push**

```bash
git commit -m "feat: add my feature"
git push origin feature/my-feature
```

6. **Create a pull request** on GitHub

### Common Development Tasks

#### Debug API Calls

Enable trace logging to see all HTTP requests:

```bash
cargo run -- --log-level trace
```

#### Add New Color Map

Edit `src/resources/common.json`:

```json
"color_maps": {
  "my_status": [
    { "value": "ACTIVE", "color": [0, 255, 0] },
    { "value": "INACTIVE", "color": [128, 128, 128] }
  ]
}
```

#### Add Sub-Resource Navigation

In the parent resource definition:

```json
"sub_resources": [
  {
    "resource_key": "child-resource",
    "display_name": "Children",
    "shortcut": "c",
    "parent_id_field": "name",
    "filter_param": "parent"
  }
]
```

#### Add Resource Action

```json
"actions": [
  {
    "key": "a",
    "display_name": "Do Action",
    "shortcut": "a",
    "sdk_method": "do_action",
    "confirm": {
      "message": "Perform action",
      "default_yes": false,
      "destructive": false
    }
  }
]
```

---

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_registry_loads_successfully

# Run with output
cargo test -- --nocapture
```

---

## Contributing

We welcome contributions! Please follow these guidelines:

### Code Style

- Follow Rust standard conventions (use `cargo fmt`)
- Run `cargo clippy` and address all warnings
- Add tests for new features
- Update documentation as needed

### Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes with clear, descriptive commits
4. Ensure all tests pass (`cargo test`)
5. Update CLAUDE.md if you change architecture or add features
6. Submit a pull request with a clear description

### Commit Message Format

Follow conventional commits:

```
feat: add support for Cloud Run services
fix: resolve authentication timeout issue
docs: update installation instructions
chore: bump dependencies to latest versions
```

---

## Release Process

### Version Management

Version is injected at compile time via `TGCP_VERSION` environment variable (for CI/CD). Local builds show "dev" version.

### Creating a Release

Releases are automated via GitHub Actions. To create a new release:

1. **Update version** in `Cargo.toml`:

```toml
[package]
version = "0.2.0"
```

2. **Commit and push** the version bump:

```bash
git add Cargo.toml
git commit -m "chore: bump version to 0.2.0"
git push origin main
```

3. **Create and push a git tag**:

```bash
git tag v0.2.0
git push origin v0.2.0
```

4. **GitHub Actions will automatically**:
   - Create a draft release on GitHub
   - Build binaries for all platforms:
     - Linux (x86_64 GNU, x86_64 musl, ARM64)
     - macOS (x86_64, ARM64/Apple Silicon)
     - Windows (x86_64)
   - Upload binaries with SHA256 checksums
   - Publish the release

### Manual Release (workflow_dispatch)

You can also trigger a release manually from GitHub Actions:

1. Go to **Actions** → **Release** workflow
2. Click **Run workflow**
3. Enter the tag (e.g., `v0.2.0`)
4. Click **Run workflow**

### Supported Platforms

The release workflow builds for:

| Platform | Target | Binary Name |
|----------|--------|-------------|
| Linux x86_64 (GNU) | `x86_64-unknown-linux-gnu` | `tgcp-linux-x86_64.tar.gz` |
| Linux x86_64 (musl) | `x86_64-unknown-linux-musl` | `tgcp-linux-x86_64-musl.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `tgcp-linux-aarch64.tar.gz` |
| macOS x86_64 | `x86_64-apple-darwin` | `tgcp-darwin-x86_64.tar.gz` |
| macOS ARM64 | `aarch64-apple-darwin` | `tgcp-darwin-aarch64.tar.gz` |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `tgcp-windows-x86_64.zip` |

---

## Supported GCP Resources

### Compute Engine
- VM Instances (`compute-instances`)
- Persistent Disks (`compute-disks`)
- VPC Networks (`compute-networks`)
- Subnets (`compute-subnetworks`)
- Firewall Rules (`compute-firewalls`)

### Cloud Storage
- Buckets (`storage-buckets`)
- Objects (`storage-objects`)

### GKE
- Clusters (`gke-clusters`)
- Node Pools (`gke-nodepools`)

### Cloud CDN / Load Balancing
- Backend Services (`cdn-backend-services`)
- Backend Buckets (`cdn-backend-buckets`)
- URL Maps (`cdn-url-maps`)
- HTTP Proxies (`cdn-target-http-proxies`)
- HTTPS Proxies (`cdn-target-https-proxies`)
- TCP Proxies (`lb-target-tcp-proxies`)
- SSL Proxies (`lb-target-ssl-proxies`)
- gRPC Proxies (`lb-target-grpc-proxies`)
- Forwarding Rules (`cdn-forwarding-rules`)
- SSL Certificates (`cdn-ssl-certificates`)
- Health Checks (`lb-health-checks`)
- Target Pools (`lb-target-pools`)
- SSL Policies (`lb-ssl-policies`)
- Security Policies (`lb-security-policies`) - Cloud Armor
- Network Endpoint Groups (`lb-network-endpoint-groups`)

## Troubleshooting

### "Permission denied" errors
Check your GCP IAM permissions for the resources you're trying to access.

### "Authentication failed"
Run `gcloud auth application-default login` to refresh credentials.

### TUI rendering issues
Try resizing your terminal window, or check if your terminal supports 256 colors.

### API rate limits
The app doesn't implement retry logic. Wait a moment and try again if you hit rate limits.

---

## License

tgcp is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

## Additional Resources

- **Repository**: https://github.com/mnicolet/tgcp
- **Issues**: https://github.com/mnicolet/tgcp/issues
- **Releases**: https://github.com/mnicolet/tgcp/releases
- **GCP Documentation**: https://cloud.google.com/docs
