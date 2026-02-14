---
title: Endpoints
description: REST API endpoints reference
---

# REST API Endpoints

Faber provides a simple REST API for task execution and health monitoring.

## Base URL

```
http://localhost:3000/api/v1
```

## Authentication

All endpoints except health check require authentication via the `Authorization` header:

```
Authorization: Bearer your-api-key
```

## Health Check

### GET /health

Check if the Faber server is running.

**Request:**

```bash
curl http://localhost:3000/api/v1/health
```

**Response:**

```json
{
  "status": "ok"
}
```

**Status Codes:**

| Code | Description |
|------|-------------|
| 200 | Server is healthy |

## Execute Tasks

### POST /execute

Execute a sequence of tasks.

**Request:**

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[
    {
      "cmd": "/bin/echo",
      "args": ["Hello"]
    }
  ]'
```

**Request Body:**

Array of `ExecutionStep` objects. Each step can be:

- **Single task** (object) - Executed sequentially
- **Parallel tasks** (array) - Executed concurrently

**Task Object:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cmd` | string | Yes | Command path or name |
| `args` | string[] | No | Command arguments |
| `env` | object | No | Environment variables |
| `stdin` | string | No | Standard input content |
| `files` | object | No | Files to create (path → content) |
| `working_dir` | string | No | Working directory |

**Example Request:**

```json
[
  {
    "cmd": "/bin/echo",
    "args": ["Step 1"]
  },
  {
    "cmd": "/usr/bin/gcc",
    "args": ["hello.c", "-o", "hello"],
    "files": {
      "hello.c": "#include <stdio.h>\nint main() { printf(\"Hello!\\n\"); return 0; }"
    }
  },
  [
    {
      "cmd": "/bin/echo",
      "args": ["Parallel A"]
    },
    {
      "cmd": "/bin/echo",
      "args": ["Parallel B"]
    }
  ]
]
```

**Response:**

Array of `TaskResult` or `TaskResult[]` (for parallel steps).

**Task Result:**

```json
{
  "stdout": "string",
  "stderr": "string",
  "exit_code": 0,
  "stats": {
    "memory_peak_bytes": 1048576,
    "cpu_usage_usec": 12345,
    "pids_peak": 1,
    "execution_time_ms": 15
  }
}
```

**Status Codes:**

| Code | Description |
|------|-------------|
| 200 | Success |
| 400 | Bad request (empty task group) |
| 401 | Unauthorized (missing or invalid API key) |
| 500 | Internal server error |

## Error Responses

### 400 Bad Request

```json
{
  "error": "Bad request",
  "message": "Task group cannot be empty"
}
```

### 401 Unauthorized

```json
{
  "error": "Unauthorized",
  "message": "Invalid or missing API key"
}
```

### 500 Internal Server Error

```json
{
  "error": "Internal server error",
  "message": "Container setup failed"
}
```

## Exit Codes

Task results include an `exit_code` field:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 127 | Command not found |
| 128+N | Fatal signal N |

## Caching

When `CACHE_ENABLED` is set to `true`:

- Requests are hashed using SHA256
- Duplicate requests return cached results
- Cache is in-memory only (not persisted)

**Cache Key:**

```
SHA256(serialized_task_group)
```

## Rate Limiting

Currently, Faber does not implement rate limiting. For production use, implement rate limiting at the reverse proxy level (nginx, traefik, etc.).

## Examples

### Simple Command

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[{"cmd": "/bin/echo", "args": ["Hello"]}]'
```

### With Environment Variables

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[{
    "cmd": "/bin/sh",
    "args": ["-c", "echo $GREETING"],
    "env": {"GREETING": "Hello, World!"}
  }]'
```

### With Files

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[{
    "cmd": "/usr/bin/python3",
    "args": ["script.py"],
    "files": {
      "script.py": "print('Hello from Python!')"
    }
  }]'
```

### Parallel Execution

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[
    [{"cmd": "/bin/sleep", "args": ["1"]}, {"cmd": "/bin/sleep", "args": ["1"]}],
    {"cmd": "/bin/echo", "args": ["Done!"]}
  ]'
```

## SDK Usage

See the [JavaScript SDK](/sdk/javascript/overview/) for higher-level API access.
