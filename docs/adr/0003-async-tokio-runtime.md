# ADR 0003: Async Strategy with Tokio Runtime

## Status
Accepted

## Context
tgcp needs to:
1. Make HTTP requests to GCP APIs (often slow, 100-500ms)
2. Keep UI responsive during API calls
3. Handle pagination and concurrent resource fetching
4. Poll for operation status updates

Blocking the UI thread during API calls would make the app unusable.

## Decision
We use **Tokio** as the async runtime with a single-threaded approach for the UI loop and spawn async tasks for API calls.

### Architecture
```
┌─────────────────────────────────────────────┐
│                 Main Thread                  │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐ │
│  │ Event   │───▶│   App   │───▶│   UI    │ │
│  │ Loop    │    │ State   │    │ Render  │ │
│  └─────────┘    └─────────┘    └─────────┘ │
│       │                                      │
│       ▼                                      │
│  ┌─────────────────────────────────────────┐│
│  │           Tokio Runtime                  ││
│  │  ┌───────┐  ┌───────┐  ┌───────┐       ││
│  │  │ HTTP  │  │ HTTP  │  │ Poll  │       ││
│  │  │ Task  │  │ Task  │  │ Task  │       ││
│  │  └───────┘  └───────┘  └───────┘       ││
│  └─────────────────────────────────────────┘│
└─────────────────────────────────────────────┘
```

### Key Patterns

#### 1. Non-Blocking Event Loop
```rust
loop {
    // Render UI
    terminal.draw(|f| ui::render(f, &mut app))?;

    // Poll for events with timeout (allows async tasks to progress)
    if event::poll(Duration::from_millis(100))? {
        handle_event(&mut app);
    }

    // Check async task results
    app.check_pending_operations();
}
```

#### 2. Spawn Tasks for API Calls
```rust
let client = app.client.clone();
tokio::spawn(async move {
    let result = client.get(&url).await;
    // Send result back via channel or shared state
});
```

#### 3. Concurrent Resource Fetching
```rust
pub async fn fetch_resources_concurrent(
    client: &GcpClient,
    resources: &[&ResourceDef],
) -> Vec<Result<Vec<Value>>> {
    let futures: Vec<_> = resources
        .iter()
        .map(|r| fetch_resources(client, r, None))
        .collect();

    futures::future::join_all(futures).await
}
```

#### 4. Operation Polling
```rust
async fn poll_operation_status(client: &GcpClient, op: &Operation) {
    loop {
        let status = client.get(&op.self_link).await?;
        if status["status"] == "DONE" {
            return Ok(status);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
```

## Consequences

### Positive
- **Responsive UI**: Never blocks on network I/O
- **Efficient**: Single-threaded avoids sync overhead
- **Concurrent fetching**: Parallel API calls for faster startup
- **Ecosystem**: reqwest, gcp_auth use tokio natively

### Negative
- **Complexity**: Async code harder to debug
- **State sharing**: Need careful handling of `&mut App` across async boundaries
- **Cancellation**: Must handle task cleanup on quit

### Mitigations

#### State Isolation
App state modifications happen only in main thread. Async tasks return data via:
- Cloned client (immutable)
- Results stored in pending queues
- Channels for complex flows

#### Error Propagation
```rust
// Errors from async tasks don't crash the app
match result {
    Ok(items) => app.set_items(items),
    Err(e) => app.show_error(&format!("Failed: {}", e)),
}
```

## Alternatives Considered

### 1. Multi-threaded Runtime (rejected)
- Pro: Better CPU utilization
- Con: Sync overhead for UI state, complexity

### 2. async-std (rejected)
- Pro: Similar API to std
- Con: Less ecosystem support, gcp_auth/reqwest prefer tokio

### 3. Blocking in Thread Pool (rejected)
- Pro: Simpler mental model
- Con: Thread overhead, harder cancellation

### 4. Polling without async (rejected)
- Pro: No async complexity
- Con: Would need to implement non-blocking HTTP manually

## References
- Event loop: `src/main.rs`
- Concurrent fetching: `src/resource/fetcher.rs`
- GCP client: `src/gcp/client.rs`
- Tokio docs: https://tokio.rs
