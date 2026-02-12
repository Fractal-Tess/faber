# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Faber is a secure, sandboxed task execution runtime written in Rust that runs commands in isolated containers with resource limits and monitoring. The project consists of a workspace with two main crates:

- `faber-runtime`: Core execution engine with containerization, cgroups, and namespace isolation
- `faber-api`: REST API server providing HTTP interface to the runtime
- `faber`: Main binary that ties everything together
- `sdks/js`: TypeScript/JavaScript SDK for client integration

## Development Commands

### Building

```bash
cargo build --release
```

### Development build

```bash
cargo build
```

### Running tests

```bash
# Rust tests
cargo test

# JavaScript SDK tests
cd sdks/js && npm test

# Integration tests for JS SDK
cd sdks/js && npm run test:integration
```

### Linting

```bash
# Rust linting
cargo clippy

# JavaScript SDK linting and type checking
cd sdks/js && npm run type-check
```

### Formatting

```bash
cargo fmt
```

### Development Environment

The project uses Nix for development environment setup:

```bash
nix develop
```

This provides a complete Rust toolchain including rust-analyzer, clippy, rustfmt, and bun for JavaScript SDK development.

### JavaScript SDK Development

```bash
cd sdks/js

# Install dependencies
bun install

# Build the SDK
npm run build

# Development build with watch mode
npm run dev

# Clean build artifacts
npm run clean
```

### Docker Development

Build development Docker image:

```bash
docker build -f docker/dev/Dockerfile -t faber-dev .
```

Build production Docker image:

```bash
docker build -f docker/prod/Dockerfile -t faber-prod .
```

## Architecture

### Core Components

**Runtime Layer** (`faber-runtime` crate):

- `Runtime`: Main orchestrator that executes task groups
- `TaskGroup`: Collection of tasks that can run in parallel or sequence
- `Container`: Provides isolated execution environment using Linux namespaces
- `Cgroup`: Resource management and monitoring (CPU, memory, PIDs)

**API Layer** (`faber-api` crate):

- `Router`: Axum-based HTTP routing with `/api/v1` prefix
- `ExecuteHandler`: Handles task execution requests with file management
- `Cache`: Request fingerprinting and response caching using DashMap
- `State`: Shared application state management
- `Middleware`: API key authentication and request processing

### Key Architectural Patterns

1. **Builder Pattern**: Used extensively for configuration (`RuntimeBuilder`, `ContainerConfigBuilder`, `CgroupConfigBuilder`)

2. **Result Types**: Custom result types provide detailed error information and execution statistics

3. **Resource Management**: Linux cgroups v2 for resource isolation and monitoring

4. **Security Model**:
   - Unprivileged user execution
   - Dropped capabilities
   - Namespace isolation (PID, mount, network, UTS, IPC)
   - Future: syscall filtering via seccomp

### Container Security

The runtime creates containers with:

- Separate mount namespace with minimal `/proc` and `/sys`
- Unprivileged user (UID/GID 1000:1000)
- Limited capabilities (drops most privileged capabilities)
- Resource limits via cgroups
- Timeout support for execution

### API Design

- Single `/execute` endpoint accepting JSON array of `ExecutionStep` tasks
- Tasks can be `Single` commands or `Parallel` task groups
- Each task specifies: `cmd`, `args` (optional), and `files` (optional)
- Response includes stdout, stderr, exit code, and resource usage statistics
- Request fingerprinting using SHA2 prevents duplicate executions
- File content is embedded in request/response for portability
- API key authentication required for all requests

## Configuration

Environment variables:

- `HOST`: Server host (default: 0.0.0.0)
- `PORT`: Server port (default: 3000)
- `MAX_CONCURRENCY`: Maximum concurrent executions (default: 10)
- `API_KEY`: API key for authentication (required)
- `CACHE_ENABLED`: Enable/disable request caching (default: true)

## Dependencies

Key external dependencies:

- `tokio`: Async runtime
- `axum`: HTTP framework
- `nix`: Linux system calls and low-level utilities
- `caps`: Linux capabilities manipulation
- `seccompiler`: seccomp filtering (future use)
- `dashmap`: Concurrent hashmap for caching
- `sha2`: Request fingerprinting

## Development Notes

- The project requires Linux containers and cgroups v2 support
- Running containers requires privileges (typically `--privileged` flag in Docker)
- The binary targets `x86_64-unknown-linux-musl` for static linking
- Release builds use aggressive optimization (LTO, codegen-units = 1)
- JavaScript SDK uses TypeScript with Vitest for testing and tsup for building
- API authentication is mandatory - set `API_KEY` environment variable

## Testing Strategy

- Rust unit tests: `cargo test`
- JavaScript SDK unit tests: `cd sdks/js && npm test`
- Integration tests require running Faber server and test against live API
- Integration test configuration: `sdks/js/test/integration-only.config.ts`
