# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-10-10

### Security
- SSH `extra_args`: closed `--ssh-flag` bypass; `-o` options now use an allow-list, `-F`/`-I` rejected
- `--readonly` now also blocks SSH (`x`/`X`), which can write keys to instance/project metadata
- Project/zone IDs validated everywhere (CLI, config, `:project`, `:zone`)
- Path/query parameters percent-encoded; operation polling only follows `https://*.googleapis.com`
- Log and config files created owner-only (0600, log dir 0700); unparsable config backed up to `config.json.bak`
- Pagination capped and guarded against repeated page tokens
- CI: least-privilege workflow permissions, no `${{ inputs }}` interpolation in scripts, audit runs on committed `Cargo.lock`

### Changed
- reqwest 0.13 (rustls + ring, HTTP/2, gzip); `serde_yaml` replaced by `serde_yaml_ng`; all dependencies updated
- Windows: browser opened via `rundll32` instead of `cmd /C start`; browser launch no longer blocks the UI
- Release builds use thin LTO, single codegen unit and stripped binaries

### Performance
- Screen redrawn only when something changes (idle CPU near zero)
- Describe view JSON computed once instead of every frame
- Projects, zones and first page of instances fetched concurrently at startup
- Fewer allocations and clones in sorting, filtering, JSON extraction and rendering

### Fixed
- Refresh reloaded the next page instead of the current one
- Pagination not reset when switching project or zone
- Aggregated (`zone=all`) lists dropped results after the first page
- Page numbering started at 0

### Breaking
- SSH configs using `-o` options outside the allow-list (e.g. `ProxyJump`) are rejected
- Legacy domain-scoped project IDs (`example.com:proj`) are rejected

## [0.1.0] - 2024-01-XX

### Added
- Initial release of tgcp
- Terminal UI for Google Cloud Platform resources
- Support for Compute Engine (VMs, Disks, Networks, Firewalls)
- Support for Cloud Storage (Buckets, Objects)
- Support for GKE (Clusters, Node Pools)
- Support for Cloud CDN and Load Balancing resources
- Vim-style keyboard navigation
- Theme system with 7 built-in themes
- Per-project theme configuration
- SSH integration with IAP tunnel support
- Data-driven resource definitions via JSON
- Hierarchical navigation with sub-resources
- Command mode with aliases
- Project and zone switching
- Resource filtering and pagination
- Confirmation dialogs for destructive actions

### Technical
- Built with Rust 2021 edition
- Uses ratatui 0.30 for TUI
- Async runtime with tokio
- GCP authentication via gcp_auth
- HTTP client with rustls (no OpenSSL dependency)
- Configuration stored in XDG directories
- Comprehensive logging with tracing

[Unreleased]: https://github.com/MrMichou/tgcp/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/MrMichou/tgcp/compare/v0.4.4...v0.5.0
[0.1.0]: https://github.com/mnicolet/tgcp/releases/tag/v0.1.0
