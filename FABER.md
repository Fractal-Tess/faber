# Faber - Secure Task Execution Runtime

Faber is a secure, sandboxed task execution runtime built in Rust. It uses Linux namespaces and cgroups v2 to provide isolated execution environments for running untrusted code safely. Similar to [go-judge](https://github.com/criyle/go-judge), it exposes a REST API for executing commands in sandboxed containers.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [API Reference](#api-reference)
- [Task Format](#task-format)
- [Isolation Mechanisms](#isolation-mechanisms)
- [Crate Structure](#crate-structure)
- [Configuration](#configuration)
- [Running Faber](#running-faber)
- [Development Guide](#development-guide)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         HTTP Request                            │
│                    POST /api/v1/execute                         │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        faber-api                                │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │   Router    │→ │  Middleware  │→ │   Execute Handler      │ │
│  │  (axum)     │  │  (API Key)   │  │                        │ │
│  └─────────────┘  └──────────────┘  └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      faber-runtime                              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    RuntimeBuilder                        │   │
│  │  - TaskGroup (tasks to execute)                         │   │
│  │  - Container (namespace isolation)                       │   │
│  │  - Cgroup (resource limits)                             │   │
│  │  - Timeout (execution timeout)                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                      Runtime                             │   │
│  │                                                          │   │
│  │   fork() ──► Container Setup ──► Task Execution          │   │
│  │              (namespaces,         (per-task cgroup,      │   │
│  │               pivot_root,          fork, execvpe)        │   │
│  │               bind mounts)                               │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Execution Flow

1. **API Request** → JSON task group received at `/api/v1/execute`
2. **Cache Check** → If caching enabled, check for cached result
3. **Runtime Build** → Create Runtime with Container + Cgroup configs
4. **Fork (Parent)** → Parent process forks to create isolation boundary
5. **Container Setup** → Child sets up namespaces, pivot_root, bind mounts
6. **Task Execution** → For each task:
   - Create per-task cgroup with resource limits
   - Write input files to working directory
   - Fork again, add child to cgroup
   - Apply security restrictions (drop privileges, capabilities)
   - Execute command via `execvpe()`
   - Collect stdout/stderr/exit_code
   - Measure resource usage from cgroup
7. **Result Return** → JSON response with results and stats

---

## API Reference

### Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/health` | No | Health check |
| POST | `/api/v1/execute` | Yes | Execute task group |

### Authentication

Protected endpoints require the `Authorization` header:

```
Authorization: Bearer <API_KEY>
```

### Health Check

```bash
curl http://localhost:3000/api/v1/health
```

**Response:**
```json
{"status": "ok"}
```

### Execute Tasks

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <API_KEY>" \
  -d '[{"cmd": "/bin/echo", "args": ["Hello"]}]'
```

**Response:**
```json
[
  {
    "stdout": "Hello\n",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 512000,
      "cpu_usage_usec": 1500,
      "pids_peak": 1,
      "execution_time_ms": 10
    }
  }
]
```

**Error Response:**
```json
[
  {
    "error": "Task execution failed: ...",
    "stats": {
      "memory_peak_bytes": 0,
      "cpu_usage_usec": 0,
      "pids_peak": 0,
      "execution_time_ms": 0
    }
  }
]
```

---

## Task Format

### Task Object

```typescript
interface Task {
  cmd: string;                      // Required: Command to execute (absolute path)
  args?: string[];                  // Optional: Command arguments
  env?: Record<string, string>;     // Optional: Environment variables
  stdin?: string;                   // Optional: Data to write to stdin
  files?: Record<string, string>;   // Optional: Files to create before execution
  working_dir?: string;             // Optional: Working directory (default: /faber)
}
```

### Task Group (Request Body)

A task group is an array of execution steps. Each step can be:
- **Single task**: `{...}` - Executed alone
- **Parallel tasks**: `[{...}, {...}]` - Executed concurrently

```json
[
  {"cmd": "/bin/task1"},
  [{"cmd": "/bin/parallel1"}, {"cmd": "/bin/parallel2"}],
  {"cmd": "/bin/task3"}
]
```

Execution order:
1. `task1` runs first
2. `parallel1` and `parallel2` run concurrently
3. `task3` runs after both parallel tasks complete

### Examples

**Simple echo:**
```json
[{"cmd": "/bin/echo", "args": ["Hello, World!"]}]
```

**With environment variables:**
```json
[{"cmd": "/bin/sh", "args": ["-c", "echo $GREETING"], "env": {"GREETING": "Hello"}}]
```

**With input files:**
```json
[{
  "cmd": "/bin/sh",
  "args": ["-c", "gcc -o hello hello.c && ./hello"],
  "files": {
    "hello.c": "#include <stdio.h>\nint main() { printf(\"Hello!\\n\"); return 0; }"
  }
}]
```

**With stdin:**
```json
[{"cmd": "/bin/cat", "stdin": "This is stdin input"}]
```

**Parallel execution:**
```json
[[
  {"cmd": "/bin/echo", "args": ["task1"]},
  {"cmd": "/bin/echo", "args": ["task2"]}
]]
```

---

## Isolation Mechanisms

### Linux Namespaces

Faber uses the following namespaces for isolation:

| Namespace | Flag | Purpose |
|-----------|------|---------|
| **PID** | `CLONE_NEWPID` | Process ID isolation - tasks see themselves as PID 1 |
| **Network** | `CLONE_NEWNET` | Network isolation - only loopback interface |
| **Mount** | `CLONE_NEWNS` | Filesystem isolation - pivoted root |
| **UTS** | `CLONE_NEWUTS` | Hostname isolation - hostname set to "faber" |
| **IPC** | `CLONE_NEWIPC` | IPC isolation - separate message queues, semaphores |

**Not implemented:** User namespace (`CLONE_NEWUSER`)

### Cgroup v2 Resource Limits

Each task runs in its own cgroup with configurable limits:

| Resource | Controller | Default | Description |
|----------|------------|---------|-------------|
| **CPU** | `cpu.max` | `50000 100000` | 50% CPU quota (50ms per 100ms period) |
| **Memory** | `memory.max` | `max` | Inherits from parent (configurable) |
| **PIDs** | `pids.max` | `64` | Maximum number of processes |

### Filesystem Isolation

The container root is created via `pivot_root` with:

| Path | Type | Mode | Description |
|------|------|------|-------------|
| `/bin` | bind | ro | Host binaries |
| `/lib` | bind | ro | Host libraries |
| `/lib64` | bind | ro | 64-bit libraries |
| `/usr` | bind | ro | User programs |
| `/dev` | bind | ro | Device nodes (null, zero, random, urandom) |
| `/proc` | bind | ro | Process information |
| `/sys` | mount | ro | System information |
| `/tmp` | tmpfs | rw | Temporary directory (128MB) |
| `/faber` | tmpfs | rw | Working directory (128MB) |

### Security Hardening

1. **UID/GID drop**: Tasks run as `nobody` (65534:65534)
2. **Capability drop**: All capabilities are dropped
3. **Seccomp**: Placeholder for syscall filtering (not yet implemented)

---

## Crate Structure

```
crates/
├── faber-api/           # HTTP API layer
│   └── src/
│       ├── lib.rs       # Public exports
│       ├── router.rs    # Route definitions
│       ├── serve.rs     # Server startup
│       ├── state.rs     # Application state
│       ├── cache.rs     # Result caching
│       ├── middleware.rs # API key auth
│       └── handlers/
│           ├── mod.rs
│           ├── health.rs
│           └── execute.rs
│
└── faber-runtime/       # Execution runtime
    └── src/
        ├── lib.rs       # Public exports
        ├── prelude.rs   # Common imports
        ├── error.rs     # Error types (FaberError)
        ├── task.rs      # Task, TaskGroup, ExecutionStep
        ├── result.rs    # TaskResult, RuntimeResult
        ├── utils.rs     # Helper functions
        ├── container/
        │   ├── mod.rs
        │   ├── core.rs    # Container setup (namespaces, mounts)
        │   ├── config.rs  # ContainerConfig
        │   └── builder.rs # ContainerConfigBuilder
        ├── cgroup/
        │   ├── mod.rs
        │   ├── core.rs    # Cgroup hierarchy management
        │   ├── config.rs  # CgroupConfig
        │   ├── builder.rs # CgroupConfigBuilder
        │   └── task.rs    # Per-task cgroup (TaskCgroup)
        └── runtime/
            ├── mod.rs
            ├── core.rs    # Runtime execution logic
            └── builder.rs # RuntimeBuilder
```

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, loads config, starts server |
| `src/config.rs` | Environment variable configuration |
| `crates/faber-api/src/router.rs` | API route definitions |
| `crates/faber-api/src/handlers/execute.rs` | Task execution handler |
| `crates/faber-runtime/src/runtime/core.rs` | Core execution logic |
| `crates/faber-runtime/src/container/core.rs` | Namespace/mount setup |
| `crates/faber-runtime/src/cgroup/task.rs` | Per-task cgroup management |

---

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `API_KEY` | Yes | - | API key for authentication |
| `PORT` | No | `3000` | Server port |
| `HOST` | No | `0.0.0.0` | Server bind address |
| `MAX_CONCURRENCY` | No | `10` | Max concurrent requests |
| `CACHE_ENABLED` | No | `true` | Enable result caching |

### Runtime Defaults

```rust
// Container defaults (ContainerConfig)
container_root_dir: /tmp/faber/<random_id>
workdir: /faber
tmpdir_size: 128M
workdir_size: 128M
hostname: faber
bind_mounts_ro: ["/bin", "/lib", "/lib64", "/usr"]

// Cgroup defaults (CgroupConfig)
cpu_max: "50000 100000"  // 50% CPU
memory_max: "max"        // Inherit from parent
pids_max: 64

// Runtime defaults (RuntimeBuilder)
timeout: 5 seconds
```

---

## Running Faber

### Prerequisites

- Linux kernel with cgroups v2
- Root privileges (or appropriate capabilities)
- Cgroup hierarchy at `/sys/fs/cgroup/faber`

### Host Setup

```bash
# Create faber cgroup directory
sudo mkdir -p /sys/fs/cgroup/faber
sudo chmod 777 /sys/fs/cgroup/faber

# Enable required controllers
echo "+cpu +memory +pids" | sudo tee /sys/fs/cgroup/faber/cgroup.subtree_control
```

### Docker Deployment

```bash
# Build the image
docker build -f docker/prod/Dockerfile -t faber:latest .

# Run with required privileges
sudo docker run -d \
    --name faber \
    --privileged \
    --cgroupns=host \
    -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
    -e API_KEY=your-secret-key \
    -p 3000:3000 \
    faber:latest
```

**Required Docker flags:**
- `--privileged`: Required for namespace operations
- `--cgroupns=host`: Use host cgroup namespace
- `-v /sys/fs/cgroup:/sys/fs/cgroup:rw`: Mount cgroup filesystem

### With GCC (for code compilation)

Use `docker/prod-gcc/Dockerfile` which includes `gcc` and `libc6-dev`.

---

## Development Guide

### Building

```bash
cargo build --release
```

### Testing

```bash
# Health check
curl http://localhost:3000/api/v1/health

# Execute task
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer test-key" \
  -d '[{"cmd": "/bin/echo", "args": ["test"]}]'
```

### Using the Runtime Programmatically

```rust
use faber_runtime::{RuntimeBuilder, Task, ExecutionStep};
use std::time::Duration;

let task = Task {
    cmd: "/bin/echo".to_string(),
    args: Some(vec!["Hello".to_string()]),
    env: None,
    stdin: None,
    files: None,
    working_dir: None,
};

let runtime = RuntimeBuilder::default()
    .with_task_group(vec![ExecutionStep::Single(task)])
    .with_timeout(Duration::from_secs(10))
    .build();

let result = runtime.execute()?;
```

### Custom Cgroup Limits

```rust
use faber_runtime::{RuntimeBuilder, CgroupConfigBuilder};

let cgroup_config = CgroupConfigBuilder::new()
    .with_cpu("100000 100000".to_string())  // 100% CPU
    .with_memory("256M".to_string())         // 256MB memory
    .with_pids(32)                           // 32 processes max
    .build();

let runtime = RuntimeBuilder::default()
    .with_task_group(tasks)
    .with_cgroup_config(cgroup_config)
    .build();
```

### Custom Container Config

```rust
use faber_runtime::ContainerConfigBuilder;
use std::path::PathBuf;

let container_config = ContainerConfigBuilder::new()
    .with_hostname("sandbox".to_string())
    .with_workdir(PathBuf::from("/workspace"))
    .with_workdir_size("256M".to_string())
    .with_tmpdir_size("64M".to_string())
    .build();

let runtime = RuntimeBuilder::default()
    .with_task_group(tasks)
    .with_container_config(container_config)
    .build();
```

---

## Known Limitations

1. **No User Namespace**: Tasks run with host UID mapping (mitigated by dropping to nobody)
2. **No Seccomp**: Syscall filtering is not yet implemented
3. **No File Persistence**: Files don't persist between tasks in a task group
4. **No Network**: Network namespace is completely isolated (no external access)
5. **Linux Only**: Requires Linux kernel with namespace and cgroup support

---

## Error Handling

The runtime uses `FaberError` enum for error types:

| Error | Description |
|-------|-------------|
| `Fork` | Failed to fork process |
| `Unshare` | Failed to create namespace |
| `Mount` | Failed to mount filesystem |
| `PivotRoot` | Failed to pivot root |
| `TaskTimeout` | Task exceeded time limit |
| `CgroupControllers` | Failed to enable cgroup controllers |
| `WriteFile` | Failed to write input file |

Task results can be:
- `TaskResult::Completed` - Success with stdout, stderr, exit_code, stats
- `TaskResult::Failed` - Error with message and stats

---

## Comparison with go-judge

| Feature | Faber | go-judge |
|---------|-------|----------|
| Language | Rust | Go |
| API | REST | REST, gRPC, WebSocket |
| Namespaces | PID, Net, Mount, UTS, IPC | PID, Net, Mount, UTS, IPC, User |
| Cgroups | v2 | v1 and v2 |
| File I/O | `files` field | `copyIn`, `copyOut`, `copyOutCached` |
| Streaming | No | WebSocket |
| File Store | No | Yes |

---

## Security Considerations

1. **Always run in Docker** with `--privileged` for production
2. **Use strong API keys** - the API key is the only authentication
3. **Set appropriate resource limits** via cgroup config
4. **Monitor container logs** for security events
5. **Keep bind mounts read-only** - only `/tmp` and `/faber` are writable
