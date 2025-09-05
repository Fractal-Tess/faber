# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

Check code in the entire workspace:

```bash
cargo check
```

## Architecture

Faber is a Rust workspace with a modular architecture consisting of:

### Workspace Structure

- **Root binary (`faber`)**: Main server entry point that starts the API server
- **`faber-api`**: REST API server crate providing HTTP endpoints using Axum
- **`faber-runtime`**: Core runtime execution engine for task processing

### Key Components

**API Layer (`faber-api`)**:

- Axum-based HTTP server with JSON endpoints
- Routes mounted at `/api/v1/`
- Handlers: `/health` (GET) and `/execute` (POST)
- Server configuration through environment variables (`PORT`, `HOST`)

**Runtime Layer (`faber-runtime`)**:

- Task execution system supporting single and parallel execution steps
- `TaskGroup`: Vector of `ExecutionStep`s that can be either `Single(Task)` or `Parallel(Vec<Task>)`
- `Task`: Command execution specification with cmd, args, env_vars, stdin, files, timeout, and working_dir
- Custom serialization/deserialization for flexible JSON input formats

### Data Flow

1. HTTP requests come into the API server
2. `/execute` endpoint accepts JSON task groups
3. Tasks are deserialized into execution steps (currently just logged, not executed)
4. Responses confirm acceptance of task groups

### Environment Configuration

- `PORT`: Server port (default: 3000)
- `HOST`: Server host (default: 0.0.0.0)

## Development Notes

The execute handler in `faber-api/src/handlers/execute.rs:24` currently only accepts and logs tasks but doesn't execute them - this appears to be a work in progress.
