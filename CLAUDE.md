# CLAUDE.md - tgcp Development Guide

## Project Overview

**tgcp** (Terminal UI for GCP) is a Rust-based terminal user interface for navigating, observing, and managing Google Cloud Platform resources. It provides a vim-style keyboard-driven experience for interacting with GCP services directly from the terminal.

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

## Build & Run

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
| `r` | Reset instance |
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

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_registry_loads_successfully

# Run with output
cargo test -- --nocapture
```

## Common Development Tasks

### Debug API Calls
Enable trace logging to see all HTTP requests:
```bash
cargo run -- --log-level trace
```

### Add New Color Map
Edit `src/resources/common.json`:
```json
"color_maps": {
  "my_status": [
    { "value": "ACTIVE", "color": [0, 255, 0] },
    { "value": "INACTIVE", "color": [128, 128, 128] }
  ]
}
```

### Add Sub-Resource Navigation
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

### Add Resource Action
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

## GCP Authentication

The app uses Application Default Credentials (ADC). Ensure you're authenticated:
```bash
gcloud auth application-default login
```

The app reads default project/zone from gcloud config:
```bash
gcloud config get-value project
gcloud config get-value compute/zone
```

## Version Management

Version is injected at compile time via `TGCP_VERSION` environment variable (for CI/CD). Local builds show "dev" version.

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
