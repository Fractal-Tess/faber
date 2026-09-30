# Faber Rust Backend

**Crates**: `faber-runtime`, `faber-api`, `faber-store`
**Purpose**: Container isolation, task execution, HTTP API

---

## Overview

The Rust backend consists of two crates in a Cargo workspace:

- **`faber-runtime`**: Core execution engine with Linux namespace isolation and cgroup v2 resource management
- **`faber-api`**: HTTP API server built with Axum, handling routing, auth, and caching
- **`faber-store`**: Content-addressed memory, filesystem, and hybrid file storage

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
        │   ├── core.rs      # Cgroup hierarchy and aggregate limits
        │   ├── request.rs   # Per-request cgroup subtree and recursive kill
        │   └── task.rs      # Per-task cgroup management
        ├── container/       # Namespace isolation
        │   ├── core.rs      # Container struct, pivot_root
        ├── runtime/         # Task execution engine
        │   ├── core.rs      # Runtime: controller, sandbox setup, task supervision
        │   ├── jailer.rs    # Re-executed jailer process and its job
        │   ├── identity.rs  # Per-request host UID/GID leases
        │   └── builder.rs   # Builder pattern
        ├── task.rs          # Task, ExecutionStep definitions
        ├── result.rs        # TaskResult, RuntimeResult
        ├── error.rs         # FaberError types
        └── utils.rs         # Helper functions
```

---

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Add API endpoint | `faber-api/src/handlers/` | Add handler + router |
| Change auth | `faber-api/src/middleware.rs` | Authorization header validation |
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
pub type Result<T> = std::result::Result<T, FaberError>;

pub enum FaberError {
    ContainerSetup(String),
    CgroupError(String),
    ExecutionFailed(String),
    // ...
}
```

### Task Execution Flow
1. `Runtime::execute()` leases a host identity and creates the request cgroup
2. It starts the jailer: the same executable again (`/proc/self/exe`, empty
   environment), which takes over before `main` and reads the job from stdin
3. The jailer sets up the container (namespaces, pivot_root)
4. For each task of an `ExecutionStep` the jailer forks a supervisor, which
   creates the task's PID namespace and init, then: fork → cgroup join →
   privilege drop → exec → wait → collect stats. Parallel tasks run side by side
5. The controller reads the result, then cleans up the container and cgroups

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
- **Namespaces**: mount, network, UTS, IPC per request; PID and user per task
- **User**: 65534 inside the user namespace, a per-request host UID/GID (100000+) outside
- **Capabilities**: All dropped via `capset`
- **Filesystem**: pivot_root to minimal rootfs

### Cgroup v2 Requirements
```rust
// Cgroup path format: one request cgroup per execution, one task cgroup per task,
// beneath the cgroup the service was started in
<container cgroup>/faber/req-{id}/task-{id}/
```

### API Authentication
```rust
// Header format (primary)
Authorization: Bearer <api_key>

// Header format (alternative)
Authorization: <api_key>

```

---

## Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_tests

# Run server
cargo run
```

Run these inside the dev container or the VM (`scripts/dev.sh`, `scripts/vm.sh`),
never on the host. Faber creates its cgroups inside the container's own cgroup (`<container cgroup>/faber/req-*/task-*`); nothing has to be created on the host.

---

## Anti-Patterns

**NEVER:**
- Use `unwrap()` in production code
- Block the async runtime with sync I/O
- Forget to cleanup cgroup directories

**ALWAYS:**
- Handle all error cases explicitly
- Use builder pattern for complex config
- Clean up resources in Drop impls
- Validate API key before any processing
