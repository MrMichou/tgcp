# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Standardized CLAUDE.md with proper structure and sections
- Added comprehensive release process documentation
- Added CHANGELOG.md for tracking releases

### Changed
- Improved CLAUDE.md organization with table of contents
- Enhanced Getting Started section with installation instructions

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

[Unreleased]: https://github.com/mnicolet/tgcp/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mnicolet/tgcp/releases/tag/v0.1.0
