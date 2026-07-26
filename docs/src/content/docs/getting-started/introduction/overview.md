---
title: Overview
description: Introduction to Faber - secure task execution runtime
---

# What is Faber?

Faber is a **secure task execution runtime** built in Rust that runs commands in isolated Linux containers with strict resource limits, comprehensive monitoring, and robust security controls.

## Key Features

- **🔒 Secure Isolation** - Linux namespaces and cgroups for complete process isolation
- **📊 Resource Monitoring** - Real-time tracking of CPU, memory, and process usage  
- **⚡ Parallel Execution** - Run multiple tasks concurrently with full isolation
- **🎯 Flexible API** - RESTful API with support for sequential and parallel task groups
- **🚀 High Performance** - Built with Rust for minimal overhead
- **📦 Docker Ready** - Easy deployment with containerized builds

## How It Works

Faber creates an isolated execution environment for every task:

1. **Container Setup** - Creates isolated namespace (PID, mount, network, UTS, IPC)
2. **Resource Limits** - Applies cgroup v2 limits for CPU, memory, and PIDs
3. **Task Execution** - Forks and executes commands with monitoring
4. **Stats Collection** - Gathers resource usage metrics
5. **Cleanup** - Removes containers and cgroups after execution

## Architecture

Faber consists of three main components:

### `faber-runtime`
Core execution engine with:
- Linux namespace isolation
- Cgroups v2 resource management  
- Task execution and monitoring
- Container lifecycle management

### `faber-api`
HTTP API server featuring:
- RESTful endpoints
- Request caching (SHA256-based)
- Authentication middleware
- Error handling

### JavaScript SDK
Client library providing:
- TypeScript support
- TaskBuilder fluent API
- Client-side testing framework
- Promise-based interface

## Use Cases

Faber is ideal for:

- **Code Execution Platforms** - Run user-submitted code safely
- **CI/CD Pipelines** - Execute build tasks in isolation
- **Sandboxed Environments** - Test untrusted code
- **Microservices** - Execute background jobs securely
- **Educational Platforms** - Run student code submissions

## Security Model

### Container Isolation
- **Namespaces**: PID, mount, network, UTS, IPC, and per-task user namespaces
- **User**: Inner and outer UID/GID 65534 (`nobody`) with one-entry maps
- **Capabilities**: All dropped via `capset`
- **Filesystem**: pivot_root to minimal rootfs

### Resource Controls
- **Cgroups v2**: CPU, memory, PIDs limits
- **Monitoring**: Peak usage tracking
- **Enforcement**: Hard limits prevent resource abuse

## Next Steps

- [Installation Guide](/getting-started/installation/docker/) - Get Faber running
- [Quick Start](/getting-started/quick-start/first-task/) - Execute your first task
- [Core Concepts](/core-concepts/isolation/overview/) - Understand the architecture
