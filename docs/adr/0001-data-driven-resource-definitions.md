# ADR 0001: Data-Driven Resource Definitions with JSON

## Status
Accepted

## Context
tgcp needs to support multiple GCP resource types (VMs, disks, buckets, clusters, etc.), each with different:
- API endpoints
- Column definitions for table display
- Actions (start, stop, delete)
- Sub-resources (VM → disks, bucket → objects)
- Status color mappings

We needed a way to add new resource types without modifying Rust code for each one.

## Decision
We chose a data-driven approach where resource definitions are stored in JSON files that are compiled into the binary at build time using `include_str!`.

### Structure
```
src/resources/
├── common.json    # Shared color maps
├── compute.json   # Compute Engine resources
├── storage.json   # Cloud Storage resources
├── gke.json       # GKE resources
└── cdn.json       # Load balancing resources
```

### Example Resource Definition
```json
{
  "compute-instances": {
    "display_name": "VM Instances",
    "service": "compute",
    "sdk_method": "list_instances",
    "response_path": "items",
    "columns": [
      { "header": "NAME", "json_path": "name", "width": 25 },
      { "header": "STATUS", "json_path": "status", "color_map": "vm_status" }
    ],
    "actions": [
      { "key": "s", "display_name": "Start", "sdk_method": "start_instance" }
    ]
  }
}
```

### SDK Dispatch Pattern
The `sdk_dispatch.rs` module maps abstract method names to concrete REST API calls:
```rust
match method {
    "list_instances" => client.get(&client.compute_zonal_url("instances")).await,
    "start_instance" => client.post(&format!("{}/start", url), None).await,
}
```

## Consequences

### Positive
- **Extensibility**: New resource types can be added by editing JSON files
- **Consistency**: All resources follow the same patterns
- **Separation of concerns**: Display logic separate from API logic
- **No runtime file loading**: JSON compiled into binary, no file I/O errors

### Negative
- **Indirection**: Harder to trace from UI to API call
- **Type safety**: JSON parsing errors only caught at runtime (mitigated by tests)
- **Limited flexibility**: Complex resources may need code changes anyway

### Mitigations
- Comprehensive tests for registry loading
- Clear documentation in CLAUDE.md
- Fallback error handling for missing resources

## Alternatives Considered

### 1. Code Generation (rejected)
Generate Rust code from JSON/YAML at build time.
- Pro: Full type safety
- Con: Complex build setup, harder to understand

### 2. Trait-Based Resources (rejected)
Implement a `Resource` trait for each type.
- Pro: Type safe, IDE support
- Con: Significant boilerplate, code changes for each resource

### 3. External Config Files (rejected)
Load JSON at runtime from config directory.
- Pro: User customization
- Con: File not found errors, version mismatches

## References
- Resource registry: `src/resource/registry.rs`
- SDK dispatch: `src/resource/sdk_dispatch.rs`
- Resource definitions: `src/resources/*.json`
