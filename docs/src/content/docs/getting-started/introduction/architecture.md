---
title: Architecture
description: Understanding Faber's architecture and components
---

# Architecture Overview

Faber is built with a modular architecture that separates concerns between container management, task execution, and API handling.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Clients                               │
│  (JavaScript SDK, cURL, Custom HTTP)                        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    faber-api (Axum)                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Health     │  │   Execute    │  │  Authentication  │  │
│  │   Handler    │  │   Handler    │  │   Middleware     │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │    Cache     │  │  App State   │                         │
│  │   (SHA256)   │  │  (Shared)    │                         │
│  └──────────────┘  └──────────────┘                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                 faber-runtime                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │   Runtime    │  │  Container   │  │  Cgroup Manager  │  │
│  │   (Executor) │  │  (Isolation) │  │  (Resources)     │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐                         │
│  │    Task      │  │    Task      │                         │
│  │   (Config)   │  │   (Result)   │                         │
│  └──────────────┘  └──────────────┘                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Linux Kernel                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  Namespaces  │  │   Cgroups    │  │   Capabilities   │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Component Details

### faber-runtime

The core runtime crate provides:

#### Runtime (`runtime/core.rs`)
- Main execution engine
- Manages container lifecycle
- Coordinates task execution
- Collects results

#### Container (`container/core.rs`)
- Sets up Linux namespaces
- Configures pivot_root
- Manages filesystem isolation
- Handles cleanup

#### CgroupManager (`cgroup/core.rs`)
- Creates cgroup hierarchies
- Applies resource limits
- Monitors usage statistics
- Cleans up on completion

### faber-api

The HTTP API crate provides:

#### Handlers (`handlers/`)
- `health.rs` - Health check endpoint
- `execute.rs` - Task execution endpoint

#### Middleware (`middleware.rs`)
- API key validation
- Authentication enforcement

#### Cache (`cache.rs`)
- SHA256-based request hashing
- In-memory result storage
- Cache hit/miss tracking

### JavaScript SDK

The client SDK provides:

#### FaberClient (`client/faber-client.ts`)
- HTTP client with authentication
- Response normalization
- Error handling

#### TaskBuilder (`builders/task-builder.ts`)
- Fluent API for task construction
- Sequential and parallel steps
- Test integration

#### Test Framework (`utils/`)
- `test-runner.ts` - Assertion execution
- `test-result-analyzer.ts` - Result reporting

## Execution Flow

### Single Task Execution

```
1. API receives POST /api/v1/execute
2. Authentication middleware validates API key
3. Cache is checked for existing result
4. RuntimeBuilder creates Runtime with TaskGroup
5. Fork → Child process sets up container
6. Task is executed in isolated environment
7. Stats are collected
8. Result is returned and cached
9. Container and cgroups are cleaned up
```

### Parallel Task Execution

```
1. Multiple tasks are received in an array
2. Each task is forked to a separate process
3. All processes run concurrently
4. Parent waits for all children to complete
5. Results are collected and returned
```

## Security Layers

### 1. Namespace Isolation
- **PID namespace**: Process ID isolation
- **Mount namespace**: Filesystem isolation
- **Network namespace**: Network stack isolation
- **UTS namespace**: Hostname isolation
- **IPC namespace**: Inter-process communication isolation

### 2. Resource Limits
- **CPU limits**: Maximum CPU usage
- **Memory limits**: Maximum RAM usage
- **PID limits**: Maximum process count

### 3. Capability Dropping
- All capabilities are dropped after setup
- Tasks run with minimal privileges

### 4. Unprivileged Execution
- Tasks execute as UID 65534 (nobody)
- No root access within containers

## Data Flow

```
Request → JSON Parsing → Auth Check → Cache Lookup
                                               ↓
Response ← JSON Response ← Result Collection ← Runtime Execution
                                               ↓
                                   Container Setup → Task Fork
                                               ↓
                                   Cgroup Setup → Exec → Wait
```

## Performance Considerations

- **Caching**: SHA256-based request caching reduces duplicate executions
- **Async Runtime**: Uses Tokio for async I/O
- **Spawn Blocking**: CPU-intensive tasks run in blocking threads
- **Zero-Copy**: Minimal data cloning where possible

## Extension Points

Faber is designed for extensibility:

- **Custom Middleware**: Add authentication providers
- **Custom Cgroup Controllers**: Support additional resource types
- **SDK Extensions**: Build clients in other languages
- **Runtime Plugins**: Add custom execution behaviors
