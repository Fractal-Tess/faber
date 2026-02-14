---
title: First Task
description: Execute your first task with Faber
---

# Your First Task

Let's execute a simple task with Faber to get familiar with the API.

## Prerequisites

- Faber running (see [Docker Installation](/getting-started/installation/docker/))
- `curl` or HTTP client
- API key (set via `API_KEY` environment variable)

## Health Check

First, verify Faber is running:

```bash
curl http://localhost:3000/api/v1/health
```

Expected response:

```json
{
  "status": "ok"
}
```

## Execute a Simple Command

Run a basic echo command:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[
    {
      "cmd": "/bin/echo",
      "args": ["Hello, Faber!"]
    }
  ]'
```

Expected response:

```json
[
  {
    "stdout": "Hello, Faber!\n",
    "stderr": "",
    "exit_code": 0,
    "stats": {
      "memory_peak_bytes": 1048576,
      "cpu_usage_usec": 12345,
      "pids_peak": 1,
      "execution_time_ms": 15
    }
  }
]
```

## Understanding the Response

### Task Result Fields

| Field | Type | Description |
|-------|------|-------------|
| `stdout` | string | Standard output from the command |
| `stderr` | string | Standard error from the command |
| `exit_code` | number | Exit code (0 = success) |
| `stats` | object | Resource usage statistics |

### Statistics Fields

| Field | Type | Description |
|-------|------|-------------|
| `memory_peak_bytes` | number | Peak memory usage in bytes |
| `cpu_usage_usec` | number | CPU usage in microseconds |
| `pids_peak` | number | Peak process count |
| `execution_time_ms` | number | Total execution time |

## Execute Multiple Tasks

Run tasks sequentially:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-api-key" \
  -d '[
    {
      "cmd": "/bin/echo",
      "args": ["Task 1"]
    },
    {
      "cmd": "/bin/echo",
      "args": ["Task 2"]
    },
    {
      "cmd": "/bin/echo",
      "args": ["Task 3"]
    }
  ]'
```

## Using the JavaScript SDK

Install the SDK:

```bash
npm install @faber/runtime-sdk
```

Execute a task:

```typescript
import { FaberClient } from '@faber/runtime-sdk';

const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'your-api-key',
});

const result = await client.executeSingle({
  cmd: '/bin/echo',
  args: ['Hello from SDK!'],
});

console.log(result.stdout); // "Hello from SDK!\n"
console.log(result.exitCode); // 0
console.log(result.stats); // Resource statistics
```

## Next Steps

- [Learn about file operations](/examples/patterns/files/)
- [Execute parallel tasks](/examples/patterns/parallel/)
- [Set up environment variables](/examples/patterns/environment/)
- [Build complex task sequences](/sdk/taskbuilder/overview/)
