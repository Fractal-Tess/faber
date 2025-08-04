# Faber - Secure Sandboxed Task Execution Service

Faber is a secure, sandboxed task execution service that provides isolated environments for running untrusted code. It uses Linux namespaces, cgroups, and seccomp to create secure containers for task execution.

## Architecture

The project is organized as a Rust workspace with multiple crates:

- **`faber-core`**: Core types, traits, and error definitions
- **`faber-config`**: Configuration management and validation
- **`faber-sandbox`**: Container and sandboxing functionality
- **`faber-executor`**: Task execution logic
- **`faber-api`**: HTTP API layer with Axum
- **`faber-cli`**: Command-line interface with Clap

## Quick Start

### Prerequisites

- Rust 1.70+
- Linux with cgroups v2 support
- Root privileges (for sandboxing features)

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd faber

# Build the project
cargo build --release

# Run the server
./target/release/faber serve
```

### CLI Usage

The Faber CLI provides several commands for managing the service:

```bash
# Start the server (default behavior)
faber serve

# Start with graceful shutdown
faber serve --graceful-shutdown

# Validate configuration
faber validate config.yaml

# Show current configuration
faber config

# Show default configuration
faber config --default

# Execute a test task
faber execute "echo hello world"

# Run with custom options
faber --config custom.yaml --host 0.0.0.0 --port 9000 serve
```

### Configuration

Faber can be configured via:

1. **Configuration file** (`config.yaml` by default)
2. **Environment variables** (prefixed with `FABER_`)
3. **Command-line arguments**

Example configuration:

```yaml
server:
  host: '127.0.0.1'
  port: 8080
  enable_swagger: true

auth:
  api_key: 'your-secret-key'
  open_mode: false

security:
  default_security_level: 'medium'
  seccomp:
    enabled: true
    level: 'medium'

resource_limits:
  default:
    memory_limit: 536870912 # 512MB
    cpu_time_limit: 30000000000 # 30 seconds
    wall_time_limit: 60000000000 # 60 seconds
    max_processes: 10
```

### API Usage

Once running, Faber exposes a REST API:

```bash
# Health check
curl http://localhost:8080/health

# Execute tasks (with authentication)
curl -X POST http://localhost:8080/execute \
  -H "Authorization: Bearer your-secret-key" \
  -H "Content-Type: application/json" \
  -d '{
    "tasks": [
      {
        "command": "echo",
        "args": ["hello", "world"],
        "env": {"CUSTOM_VAR": "value"}
      }
    ]
  }'
```

## Development

### Building

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p faber-cli

# Build with debug info
cargo build --debug
```

### Testing

```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p faber-core

# Run tests with output
cargo test -- --nocapture
```

### Development Server

```bash
# Run in development mode
cargo run -p faber-cli -- serve --debug

# Run with hot reload (if implemented)
cargo run -p faber-cli -- serve --debug --hot-reload
```

## Security Features

- **Linux Namespaces**: PID, mount, network, IPC, UTS, user, time, and cgroup namespaces
- **cgroups**: Resource limits for CPU, memory, and process count
- **seccomp**: System call filtering
- **Capability dropping**: Reduced privilege execution
- **Read-only filesystem**: Immutable root filesystem
- **Command validation**: Blocked dangerous commands

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run the test suite
6. Submit a pull request

## License

[Add your license information here]
