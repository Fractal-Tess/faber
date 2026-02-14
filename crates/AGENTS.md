# Faber Rust Backend

**Crates**: `faber-runtime`, `faber-api`  
**Purpose**: Container isolation, task execution, HTTP API

---

## Overview

The Rust backend consists of two crates in a Cargo workspace:

- **`faber-runtime`**: Core execution engine with Linux namespace isolation and cgroup v2 resource management
- **`faber-api`**: HTTP API server built with Axum, handling routing, auth, and caching

---

## Structure

```
crates/
├── faber-api/
│   └── src/
│       ├── lib.rs           # Public API exports
│       ├── cache.rs         # SHA256-based request caching
│       ├── handlers/        # HTTP route handlers
│       │   ├── health.rs    # GET /health
│       │   └── execute.rs   # POST /execute
│       ├── middleware.rs    # API key authentication
│       ├── router.rs        # Axum route definitions
│       ├── serve.rs         # Server initialization
│       └── state.rs         # AppState with cache
│
└── faber-runtime/
    └── src/
        ├── lib.rs           # Public API exports
        ├── cgroup/          # Cgroups v2 resource limits
        │   ├── core.rs      # CgroupManager
        │   └── task.rs      # Per-task cgroup management
        ├── container/       # Namespace isolation
        │   ├── core.rs      # Container struct, pivot_root
        │   └── setup.rs     # Namespace setup
        ├── runtime/         # Task execution engine
        │   ├── core.rs      # Runtime, RuntimeBuilder
        │   └── builder.rs   # Builder pattern
        ├── task.rs          # Task, ExecutionStep definitions
        ├── result.rs        # TaskResult, RuntimeResult
        ├── error.rs         # RuntimeError types
        └── utils.rs         # Helper functions
```

---

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Add API endpoint | `faber-api/src/handlers/` | Add handler + router |
| Change auth | `faber-api/src/middleware.rs` | Header/query validation |
| Modify caching | `faber-api/src/cache.rs` | SHA256 keys |
| Container setup | `faber-runtime/src/container/core.rs` | pivot_root, mounts |
| Resource limits | `faber-runtime/src/cgroup/` | CPU, memory, PIDs |
| Task execution | `faber-runtime/src/runtime/core.rs` | Fork, exec, wait |
| Error handling | `faber-runtime/src/error.rs` | Custom error types |

---

## Key Patterns

### Builder Pattern
```rust
// Runtime construction
let runtime = RuntimeBuilder::new()
    .cgroup_config(cgroup_config)
    .container_config(container_config)
    .build()?;
```

### Error Handling
```rust
pub type Result<T> = std::result::Result<T, RuntimeError>;

pub enum RuntimeError {
    ContainerSetup(String),
    CgroupError(String),
    ExecutionFailed(String),
    // ...
}
```

### Task Execution Flow
1. `Runtime::execute()` receives `TaskGroup`
2. Fork → child process sets up container (namespaces, pivot_root)
3. For each `ExecutionStep`:
   - Single task: fork → cgroup setup → exec → wait → collect stats
   - Parallel tasks: spawn all → wait all → collect results
4. Cleanup container and cgroups
5. Return `TaskGroupResult`

---

## Conventions

- **Module exports**: `lib.rs` re-exports public API only
- **Naming**: `snake_case` files, `PascalCase` types
- **Error propagation**: Use `?` operator with custom errors
- **Unsafe code**: Minimize; document safety invariants
- **Tests**: Integration tests in `faber-runtime/tests/`

---

## Critical Implementation Details

### Container Isolation
- **Namespaces**: PID, mount, network, UTS, IPC
- **User**: Unprivileged (UID/GID 65534 - nobody)
- **Capabilities**: All dropped via `capset`
- **Filesystem**: pivot_root to minimal rootfs

### Cgroup v2 Requirements
```rust
// Cgroup path format
/sys/fs/cgroup/faber/task-{id}/

// Required controllers
echo "+cpu +memory +pids" > /sys/fs/cgroup/faber/cgroup.subtree_control
```

### API Authentication
```rust
// Header format (primary)
Authorization: Bearer <api_key>

// Header format (alternative)
Authorization: <api_key>

// Query param (also supported)
?api_key=<key>
```

---

## Testing

```bash
# Unit tests
cargo test

# Integration tests (requires cgroup setup)
cargo test --test integration_tests

# Run server
cargo run
```

**Cgroup Setup Required:**
```bash
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control
```

---

## Anti-Patterns

**NEVER:**
- Run without cgroup setup (ENOMEM errors)
- Use `unwrap()` in production code
- Block the async runtime with sync I/O
- Forget to cleanup cgroup directories

**ALWAYS:**
- Handle all error cases explicitly
- Use builder pattern for complex config
- Clean up resources in Drop impls
- Validate API key before any processing
