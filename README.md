# Faber

A Rust-based task execution API server with support for single and parallel command execution.

## Development with Docker

### Running the Development Environment

1. Start the development container:

```bash
sudo docker compose -f docker/dev/docker-compose.yaml up --build -d
```

1. Execute into the container:

```bash
sudo docker exec -it faber-dev bash
```

1. Run the application with auto-reload:

```bash
cargo watch -x run
```

### Testing the API

Use an API client like Postman, Insomnia, Thunder Client, or curl to interact with the endpoints:

- **Health Check**: `GET http://localhost:3000/api/v1/health`
- **Execute Tasks**: `POST http://localhost:3000/api/v1/execute`

Example task execution request:

```json
[
  {
    "cmd": "echo",
    "args": ["Hello, World!"]
  }
]
```

## Architecture

- **faber-api**: REST API server using Axum
- **faber-runtime**: Core task execution engine
- **faber**: Main binary that starts the server
