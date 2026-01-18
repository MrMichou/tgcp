# ADR 0002: Ratatui as TUI Framework

## Status
Accepted

## Context
tgcp requires a terminal user interface framework that supports:
- Table rendering with scrolling
- Vim-style keyboard navigation
- Modal dialogs and overlays
- Cross-platform terminal support (Linux, macOS)
- Efficient re-rendering for 60fps updates

## Decision
We chose **ratatui** (v0.30) with **crossterm** (v0.29) as the backend.

### Why Ratatui
1. **Immediate mode rendering**: Simple mental model, UI is a function of state
2. **Widget composability**: Built-in Table, Paragraph, Block widgets
3. **Active development**: Regular releases, responsive maintainers
4. **Community**: Large ecosystem, many examples and tutorials
5. **Performance**: Only redraws changed regions

### Why Crossterm (over termion)
1. **Cross-platform**: Works on Windows (future-proofing)
2. **No external dependencies**: Pure Rust, no libc requirements
3. **Better event handling**: Consistent keyboard event parsing
4. **Active maintenance**: More frequent updates than termion

## Consequences

### Positive
- **Rapid development**: Rich widget library reduces custom code
- **Consistent rendering**: Framework handles terminal quirks
- **Good documentation**: Extensive examples in ratatui repo
- **Type safety**: Compile-time checks for widget composition

### Negative
- **Learning curve**: Immediate mode differs from retained GUI
- **Memory allocation**: Some widgets allocate on each frame (mitigated by virtual scrolling)
- **Limited styling**: Basic color support, no gradients/images

### Architecture Patterns

#### Immediate Mode Rendering
```rust
fn render(f: &mut Frame, app: &App) {
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(Color::DarkGray));
    f.render_stateful_widget(table, area, &mut state);
}
```

#### Virtual Scrolling
For large datasets, we only render visible rows:
```rust
let visible_range = app.scroll_offset..(app.scroll_offset + viewport_height);
let visible_rows: Vec<Row> = app.items[visible_range].iter().map(...).collect();
```

#### Mode-Based UI
```rust
match app.mode {
    Mode::Normal => render_table(f, app, area),
    Mode::Describe => render_json_view(f, app, area),
    Mode::Help => render_help_overlay(f, app),
    Mode::Command => render_command_box(f, app),
}
```

## Alternatives Considered

### 1. tui-rs (rejected)
Original library, now unmaintained in favor of ratatui fork.

### 2. cursive (rejected)
- Pro: More "traditional" retained mode API
- Con: Less flexible, harder to customize widgets

### 3. egui in terminal (rejected)
- Pro: Same API for TUI and GUI
- Con: Experimental terminal backend, worse text handling

### 4. Raw crossterm (rejected)
- Pro: Maximum control
- Con: Would need to reimplement all widgets

## References
- Main render function: `src/ui/mod.rs`
- Terminal setup: `src/main.rs`
- Ratatui docs: https://ratatui.rs
