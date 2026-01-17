# 💡 Brainstorming: Ideas for tgcp Improvements

Based on a comprehensive codebase analysis, here are concrete improvement opportunities organized by priority and impact.

## 📊 Current State Assessment

- **Total Lines of Code**: ~5,400
- **Architecture**: ✅ Excellent (modular, data-driven, clean separation of concerns)
- **Code Quality**: ✅ Very good (zero `unwrap()`, robust error handling with anyhow)
- **Security**: ✅ Solid (proper permissions, validation, security audit CI)
- **Test Coverage**: ⚠️ <5% (critical gap - only ~7 tests)
- **Performance**: ⚡ Good, but optimizable

---

## 🔴 High Priority (Immediate Impact)

### 1. Implement Full Resource Details View
- **File**: `src/app.rs:623` (existing TODO)
- **Current Issue**: Describe mode shows list data, not full resource details
- **Solution**: Call `detail_sdk_method` when entering describe mode for resources that define it
- **Impact**: Users expect complete resource information
- **Effort**: Medium

### 2. Bulk Selection and Operations
- **Current Gap**: Cannot select multiple resources
- **Proposed Features**:
  - Space bar to toggle selection
  - Visual indicators (checkbox column)
  - Bulk actions: start/stop/delete multiple VMs
  - Select all/none commands
- **Impact**: Critical for production workflows
- **Effort**: Medium

### 3. Comprehensive Test Suite ⚠️
- **Critical Finding**: Only 3 test modules for 5,400 LOC
- **Missing Coverage**:
  - Event handling (679 lines)
  - App state transitions (1,151 lines)
  - SDK dispatch (759 lines)
  - Resource fetcher (597 lines)
  - UI rendering (539 lines)
  - Config management (227 lines)
- **Target**: 80%+ coverage
- **Risk**: Regressions difficult to detect
- **Effort**: High

### 4. GCP Cost Integration
- **Feature**: Display resource costs via Cloud Billing API
- **Components**:
  - Cost per resource in table view
  - Cost trends and forecasts
  - Budget alerts
  - Cost dashboard view
- **Business Value**: Very high for cloud spend control
- **Effort**: Medium-High

---

## 🟡 Medium Priority (Significant Improvements)

### 5. Advanced Filtering
- **Current**: Simple substring search across all columns
- **Improvements**:
  - Regex support (`/vm-.*-prod/`)
  - Column-specific filters (`/status:RUNNING`)
  - Saved filters in config
  - Filter history (up/down arrows)
- **Effort**: Low-Medium

### 6. Performance: Concurrent Resource Fetching
- **Current**: Resources fetched sequentially
- **Solution**: Use `tokio::spawn` to parallelize API calls
- **Expected Gain**: 3-5x faster startup
- **Files**: `src/resource/fetcher.rs`
- **Effort**: Medium

### 7. Clipboard Integration
- **Feature**: `y` key to yank resource ID/name to clipboard
- **Implementation**: Use `arboard` crate
- **Workflow**: Copy resource name for use in other CLI tools
- **Effort**: Low

### 8. Data Export Functionality
- **Formats**: CSV, JSON, Markdown table
- **Use Cases**: Reporting, audit trails, documentation
- **Commands**: `:export csv`, `:export json`
- **Effort**: Low

### 9. Resource Templates
- **Feature**: Save resource configurations as templates
- **Operations**:
  - `:template save <name>` - Save current resource config
  - `:template apply <name>` - Clone resource with modifications
- **Benefit**: Infrastructure as Code workflow
- **Effort**: Medium

### 10. Enhanced Notifications
- **Current**: Toast notifications with polling
- **Improvements**:
  - Desktop notifications (`notify-rust`)
  - Notification filters (errors only, specific resources)
  - Searchable notification history
  - Sound alerts (configurable)
- **Effort**: Low-Medium

---

## 🟢 Low Priority (Nice to Have)

### 11. Resource Bookmarks/Favorites
- **Commands**:
  - `:bookmark <name>` - Save current resource
  - `:goto <bookmark>` - Jump to bookmarked resource
- **Storage**: Persist in `config.json`
- **Effort**: Low

### 12. Resource Comparison View
- **Feature**: Select two resources, show side-by-side diff
- **Use Case**: Compare similar instances or before/after changes
- **Effort**: Medium

### 13. Cross-Project Search
- **Feature**: Search for resources across all accessible projects
- **Command**: `:search global <query>`
- **Effort**: Medium

### 14. Offline Mode with Cache
- **Implementation**: SQLite cache with timestamp-based invalidation
- **Benefit**: Works without connectivity, faster browsing
- **Background refresh when online
- **Effort**: High

### 15. Resource Dependency Graph
- **Feature**: Visualize relationships (VM → Disk → Network → Subnet)
- **Rendering**: ASCII art or similar in TUI
- **Navigation**: Jump between related resources
- **Effort**: High

### 16. IAM Policy Management
- **Feature**: View/edit IAM policies for resources
- **Current Gap**: IAM data exists in GCP but not exposed in tgcp
- **Effort**: Medium

### 17. Cloud Shell Integration
- **Feature**: Open Cloud Shell to resource context
- **Alternative**: Complement to existing Console button (`C`)
- **Effort**: Low

### 18. Diff View for Resource Changes
- **Feature**: Show what changed between refreshes
- **Use Case**: Debugging state transitions (RUNNING → STOPPED)
- **Effort**: Medium

---

## 🏗️ Architectural Improvements

### 19. Plugin System
- **Design**: Trait-based resource providers
- **Loading**: From `~/.config/tgcp/plugins/`
- **Support**: Custom JSON schemas for resources
- **Benefit**: Community contributions without forking
- **Effort**: High

### 20. Smart Pagination
- **Current**: Manual pagination with `[` / `]`
- **Improvements**:
  - Auto-fetch when scrolling near bottom
  - Configurable page size
  - "Load all" option for small result sets
- **Effort**: Medium

### 21. Performance: Reduce Cloning Overhead
- **Issue**: Extensive use of `.clone()` for `filtered_items`, `items`
- **Solution**: Use `Arc<T>` or smart references
- **Files**: Throughout `src/app.rs`
- **Impact**: Reduced memory usage and allocations
- **Effort**: Medium

### 22. Lazy Loading with Virtual Scrolling
- **Current**: All items loaded into `filtered_items`
- **Problem**: Slow with 1000+ resources
- **Solution**: Render only visible rows
- **Effort**: High

### 23. Undo/Redo with Event Sourcing
- **Feature**: Store all state changes, allow undo with `u` key
- **Safety**: Rollback destructive actions
- **Effort**: High

---

## 🔧 Technical Debt & Maintenance

### 24. Update Deprecated Dependencies
- **Issue**: `serde_yaml` 0.9 is deprecated
- **Action**: Migrate to `serde_yml`
- **Check**: Review `gcp_auth` for updates beyond 0.12

### 25. Integration Tests
- **Implementation**: Test against GCP emulator or mocked endpoints
- **Tools**: `wiremock` for HTTP mocking
- **Coverage**: Full API interaction flows

### 26. Property-Based Testing
- **Tool**: `proptest` for fuzzing
- **Targets**: Input validation, JSON parsing, filter logic

### 27. API Documentation
- **Action**: Publish rustdoc to docs.rs
- **Enhancement**: Add examples for all public APIs

### 28. Architecture Decision Records (ADRs)
- **Purpose**: Document key design decisions
- **Topics**: Why JSON resources, why ratatui, async strategy
- **Location**: `docs/adr/`

---

## 🎯 Recommended Roadmap

### Phase 1 (Quick Wins)
1. ✅ Implement resource detail API (addresses existing TODO)
2. ✅ Add clipboard integration
3. ✅ Enhanced filtering

### Phase 2 (Foundation)
4. ✅ Comprehensive test suite (critical for stability)
5. ✅ Performance optimizations (concurrent fetching, reduce cloning)

### Phase 3 (High-Value Features)
6. ✅ Bulk operations
7. ✅ Cost integration
8. ✅ Export functionality

### Phase 4 (Extensibility)
9. ✅ Resource templates
10. ✅ Plugin system
11. ✅ Enhanced notifications

---

## 🤝 Contributing

These ideas are open for discussion and community input. Feedback welcome on:
- Priority ordering
- Implementation approaches
- Additional ideas not listed here

---

**Analysis Date**: 2026-01-17
**Codebase Version**: Based on commit `63213ef`
