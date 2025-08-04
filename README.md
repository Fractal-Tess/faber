# Faber

![Faber Logo](./faber.png)

A secure sandbox API for executing code in isolated environments. Submit source files and commands to compile, run, and interact with code in any supported language.

## Features

- 🔒 **Secure Sandbox Execution** - Each request runs in isolated temporary directories
- 🔄 **Multi-language Support** - C, Python, JavaScript, and any available language/tool
- 📁 **File Management** - Copy source files into execution environment
- ⚡ **Sequential Task Execution** - Execute commands in specified order
- 📊 **Resource Monitoring** - Track execution time and memory usage
- 🔐 **API Key Authentication** - Secure access control (can be disabled)
- 🌐 **Open Mode** - Disable authentication for public/development use
- 📖 **OpenAPI Documentation** - Swagger UI with full API documentation

## Quick Start

### Secure Mode (Default)

1. **Set Environment Variables**

   ```bash
   export API_KEY=your-secret-key
   export ENABLE_SWAGGER=true  # Optional, defaults to true
   export OPEN=false           # Optional, defaults to false
   export HOST=0.0.0.0         # Optional, defaults to 0.0.0.0
   export PORT=3000            # Optional, defaults to 3000
   ```

2. **Run the Server**

   ```bash
   cargo run
   ```

3. **Test the API (with API Key)**
   ```bash
   curl -X POST http://localhost:3000/run \
     -H "Content-Type: application/json" \
     -H "api_key: your-secret-key" \
     -d '{
       "compile": {
         "order": 0,
         "args": ["gcc", "-o", "hello", "hello.c"],
         "src": {
           "hello.c": {
             "content": "#include <stdio.h>\nint main() { printf(\"Hello World!\"); return 0; }"
           }
         }
       },
       "run": {
         "order": 1,
         "args": ["./hello"]
       }
     }'
   ```

### Open Mode (No Authentication)

1. **Enable Open Mode**

   ```bash
   export OPEN=true
   # API_KEY not required when OPEN=true
   cargo run
   ```

2. **Test the API (no API Key needed)**
   ```bash
   curl -X POST http://localhost:3000/run \
     -H "Content-Type: application/json" \
     -d '{
       "test": {
         "order": 0,
         "args": ["echo", "Public API access!"]
       }
     }'
   ```

## Debugging

Faber supports remote debugging using Docker containers with LLDB. This allows you to debug the application while it runs in an isolated environment.

### Quick Debug Setup

```bash
# Automated setup (recommended)
./dev-scripts/debug.sh

# Manual setup
cargo build
docker-compose up -d
docker-compose exec faber lldb-server platform --server --listen *:12345
```

### Debug Configurations

1. **Remote Debug (Docker Container)** - Launch and debug in container
2. **Attach to Running Process (Docker)** - Attach to existing process
3. **Local Debug (Development)** - Debug locally without Docker

### Using VSCode Debugger

1. Open VSCode in the project directory
2. Go to Run and Debug panel (Ctrl+Shift+D)
3. Select your preferred debug configuration
4. Press F5 to start debugging

### Cleanup

```bash
# Stop debugging environment
./dev-scripts/cleanup-debug.sh
```

For detailed debugging instructions, see [DEBUGGING.md](DEBUGGING.md).

## Configuration

| Environment Variable | Default    | Description                                                  |
| -------------------- | ---------- | ------------------------------------------------------------ |
| `API_KEY`            | _Required_ | API key for authentication (**not required when OPEN=true**) |
| `OPEN`               | `false`    | Disable authentication - makes all routes public             |
| `ENABLE_SWAGGER`     | `true`     | Enable/disable Swagger UI                                    |
| `HOST`               | `0.0.0.0`  | Server bind address                                          |
| `PORT`               | `3000`     | Server port                                                  |

### Authentication Modes

#### Secure Mode (OPEN=false)

```bash
export OPEN=false
export API_KEY=your-secret-key
cargo run
```

- ✅ API key required for `/run` and `/protected` endpoints
- ✅ Secure for production use
- ✅ `/health` endpoint remains public

#### Open Mode (OPEN=true)

```bash
export OPEN=true
# API_KEY not required
cargo run
```

- ⚠️ **All routes are publicly accessible**
- ⚠️ **No authentication required**
- ⚠️ **Use only for development or public APIs**

### Disabling Swagger UI

To disable Swagger UI in production:

```bash
export ENABLE_SWAGGER=false
cargo run
```

When disabled:

- `/swagger-ui/` returns 404
- `/api-docs/openapi.json` returns 404
- Main API endpoints remain fully functional

## API Endpoints

### Public Endpoints (always accessible)

- `GET /health` - Health check

### Protected Endpoints (require API key unless OPEN=true)

- `POST /run` - Execute code in sandbox
- `GET /protected` - Protected endpoint example

### Documentation Endpoints (if ENABLE_SWAGGER=true)

- `GET /swagger-ui/` - Swagger UI
- `GET /api-docs/openapi.json` - OpenAPI specification

## Request Format

```json
{
  "task_name": {
    "order": 0,
    "args": ["command", "arg1", "arg2"],
    "env": ["VAR=value"],
    "src": {
      "filename": {
        "content": "file content here"
      }
    }
  }
}
```

## Response Format

```json
{
  "task_name": {
    "exitStatus": 0,
    "time": "250ms",
    "memory": "12mb",
    "files": {
      "stdout": "output here",
      "stderr": "error output here"
    }
  }
}
```

## Examples

See [examples.md](examples.md) for detailed usage examples with different programming languages.

## Security

### Secure Mode (OPEN=false) - Recommended for Production

- **API Key Authentication**: Required for execution endpoints
- **Sandbox Isolation**: Each request runs in temporary directories
- **Process Isolation**: Commands execute in separate processes
- **Resource Limits**: Basic memory and time monitoring
- **No Persistent Storage**: Temporary files are automatically cleaned up

### Open Mode (OPEN=true) - Development/Public APIs Only

- ⚠️ **No Authentication**: All endpoints publicly accessible
- ⚠️ **Security Risk**: Should not be used for sensitive operations
- ✅ **Same Sandbox**: Still provides process and file isolation
- ✅ **Resource Limits**: Still enforced
- 🎯 **Use Cases**: Development, public code playgrounds, educational tools

## Development

```bash
# Secure development (with auth)
export API_KEY=dev-key
export OPEN=false
RUST_LOG=debug cargo run

# Open development (no auth)
export OPEN=true
RUST_LOG=debug cargo run

# Run tests
cargo test

# Check code
cargo check
```
