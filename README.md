# Faber

<div align="center">

![Faber Logo](faber.png)

**Secure, isolated task execution runtime built in Rust**

[![Build and Push Docker Image](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml/badge.svg?branch=main)](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml)

</div>

---

Faber is a high-performance task execution runtime that runs commands in isolated containers with strict resource limits, comprehensive monitoring, and robust security controls. Built with Rust for reliability and performance.

## ✨ Features

- 🔒 **Secure Isolation** - Linux namespaces and cgroups for complete process isolation
- 📊 **Resource Monitoring** - Real-time tracking of CPU, memory, and process usage
- ⚡ **Parallel Execution** - Run multiple tasks concurrently with full isolation
- 🎯 **Flexible API** - RESTful API with support for sequential and parallel task groups
- 🚀 **High Performance** - Built with Rust for minimal overhead
- 📦 **Docker Ready** - Easy deployment with containerized builds

## 🚀 Quick Start

### Prerequisites

- Docker (with privileged mode support)
- Linux kernel with cgroups v2 support

### Running Faber

1. **Build a custom image** with your required tools:

```dockerfile
FROM vgfractal/faber AS faber
FROM debian:latest

RUN apt update && apt install -y \
    gcc \
    make \
    libc-dev

WORKDIR /opt
COPY --from=faber /opt/faber /opt

EXPOSE 3000/tcp
ENTRYPOINT ["./faber"]
```

2. **Run the container**:

```bash
docker build -t my-faber .
docker run --privileged --cgroupns=host -p 3000:3000 my-faber
```

3. **Execute a task**:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "echo",
      "args": ["Hello, Faber!"]
    }
  ]'
```

## 📖 Usage Examples

### Compile and Run C Code

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "/usr/bin/gcc",
      "args": ["hello.c", "-o", "hello"],
      "files": {
        "hello.c": "#include <stdio.h>\nint main() { printf(\"Hello!\\n\"); return 0; }"
      }
    },
    {
      "cmd": "./hello"
    }
  ]'
```

### Parallel Task Execution

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "echo",
      "args": ["Task 1"]
    },
    [
      {
        "cmd": "echo",
        "args": ["Parallel A"]
      },
      {
        "cmd": "echo",
        "args": ["Parallel B"]
      }
    ]
  ]'
```

## 🔌 API Reference

### Health Check

```bash
GET /api/v1/health
```

Returns the service health status.

### Execute Tasks

```bash
POST /api/v1/execute
Content-Type: application/json
```

Execute a sequence of tasks. Each step can be:
- A single task object (executed sequentially)
- An array of task objects (executed in parallel)

**Task Fields:**
- `cmd` (required): Command to execute
- `args` (optional): Command arguments
- `env` (optional): Environment variables
- `stdin` (optional): Standard input content
- `files` (optional): Files to create (path → content mapping)
- `working_dir` (optional): Working directory

**Response:** Array of task results with `stdout`, `stderr`, `exit_code`, and `stats` (resource usage metrics).

## 🏗️ Architecture

Faber consists of three main components:

- **`faber-runtime`** - Core runtime with container isolation, cgroups, and resource monitoring
- **`faber-api`** - HTTP API server with request caching and task orchestration
- **SDKs** - Client libraries for various languages (JavaScript/TypeScript available)

## 📚 Documentation

For detailed documentation, visit the [docs site](docs/) or check out:

- [Getting Started Guide](docs/content/docs/getting-started.mdx)
- [API Reference](docs/content/docs/api-reference.mdx)
- [Configuration](docs/content/docs/configuration.mdx)
- [Examples](docs/content/docs/examples.mdx)

## 🛠️ Development

### Building from Source

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run the server
cargo run
```

### Project Structure

```
faber/
├── crates/
│   ├── faber-runtime/    # Core runtime implementation
│   └── faber-api/        # HTTP API server
├── sdks/
│   └── js/               # JavaScript/TypeScript SDK
├── docs/                 # Documentation site
└── docker/               # Docker configurations
```

## 🔐 Security

Faber implements multiple layers of security:

- **Linux Namespaces** - Process, mount, network, and user namespace isolation
- **Cgroups** - Resource limits for CPU, memory, and process counts
- **Capability Dropping** - Minimal required capabilities
- **Unprivileged Execution** - Tasks run as non-root users when possible

> **Note:** Currently requires root privileges for container setup. Authentication is not implemented - use in trusted networks or behind a reverse proxy.

## 📊 Status

### ✅ Implemented

- Container isolation (namespaces, cgroups)
- Resource monitoring and limits
- Sequential and parallel execution
- HTTP API with request caching
- JavaScript/TypeScript SDK

### 🚧 In Progress

- Syscall filtering
- Step caching
- Unprivileged execution mode

### 📋 Planned

- Additional SDKs (Python, Go, PHP, Rust)
- Enhanced documentation
- Authentication support

## 📄 License

[Add your license here]

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

**Built with ❤️ using Rust**
