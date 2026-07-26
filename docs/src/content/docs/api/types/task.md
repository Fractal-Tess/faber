---
title: Task Types
description: API type definitions for tasks and results
---

# Type Definitions

Complete reference for all API types used in Faber.

## Task

A single executable task.

```typescript
type Task = {
  cmd: string;
  args?: string[];
  env?: Record<string, string>;
  stdin?: string;
  files?: Record<string, string>;
  working_dir?: string;
  sandbox_profile?: "compile_v1" | "native_v1";
};
```

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cmd` | `string` | Yes | Command path or name |
| `args` | `string[]` | No | Command arguments |
| `env` | `Record<string, string>` | No | Environment variables |
| `stdin` | `string` | No | Standard input content |
| `files` | `Record<string, string>` | No | Workspace-relative files to create; absolute paths, `..`, symlinks, and mount traversal are rejected |
| `working_dir` | `string` | No | Working directory |
| `sandbox_profile` | `compile_v1 \| native_v1` | No | Versioned seccomp policy; defaults to `compile_v1` |

### Example

```json
{
  "cmd": "/usr/bin/gcc",
  "args": ["hello.c", "-o", "hello"],
  "env": {"CC": "gcc"},
  "files": {
    "hello.c": "#include <stdio.h>\nint main() { printf(\"Hello!\\n\"); return 0; }"
  },
  "working_dir": "/tmp",
  "sandbox_profile": "compile_v1"
}
```

## ExecutionStep

A single step in a task group.

```typescript
type ExecutionStep = Task | Task[];
```

Can be either:
- **Single Task** - Executed sequentially
- **Task Array** - Executed in parallel

### Sequential Example

```json
{
  "cmd": "/bin/echo",
  "args": ["Step 1"]
}
```

### Parallel Example

```json
[
  {"cmd": "/bin/echo", "args": ["Parallel A"]},
  {"cmd": "/bin/echo", "args": ["Parallel B"]}
]
```

## TaskGroup

A sequence of execution steps.

```typescript
type TaskGroup = ExecutionStep[];
```

### Example

```json
[
  {"cmd": "/bin/echo", "args": ["Step 1"]},
  [{"cmd": "/bin/echo", "args": ["Parallel 1"]}, {"cmd": "/bin/echo", "args": ["Parallel 2"]}],
  {"cmd": "/bin/echo", "args": ["Step 3"]}
]
```

This executes:
1. "Step 1" (sequential)
2. "Parallel 1" and "Parallel 2" (concurrent)
3. "Step 3" (sequential, after parallel completes)

## TaskResult

Result of a single task execution.

```typescript
type TaskResult = {
  stdout: string;
  stderr: string;
  exit_code: number;
  stats: ExecutionStats;
};
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `stdout` | `string` | Standard output |
| `stderr` | `string` | Standard error |
| `exit_code` | `number` | Exit code (0 = success) |
| `stats` | `ExecutionStats` | Resource statistics |

### Example

```json
{
  "stdout": "Hello, World!\n",
  "stderr": "",
  "exit_code": 0,
  "stats": {
    "memory_peak_bytes": 1048576,
    "cpu_usage_usec": 12345,
    "cpu_nr_throttled": 0,
    "cpu_throttled_usec": 0,
    "pids_peak": 1,
    "execution_time_ms": 15,
    "stdout_truncated": false,
    "stderr_truncated": false,
    "outcome": "exited",
    "termination_signal": null,
    "oom_kill_count": 0,
    "pids_limit_hit_count": 0,
    "cleanup_succeeded": true
  }
}
```

## ExecutionStats

Resource usage statistics.

```typescript
type ExecutionStats = {
  memory_peak_bytes: number;
  cpu_usage_usec: number;
  cpu_nr_throttled: number;
  cpu_throttled_usec: number;
  pids_peak: number;
  execution_time_ms: number;
  stdout_truncated: boolean;
  stderr_truncated: boolean;
  outcome: "exited" | "signaled" | "timed_out" | "out_of_memory" | "pids_limit" | "output_limit" | "policy_violation" | "infrastructure_failure";
  termination_signal: number | null;
  oom_kill_count: number;
  pids_limit_hit_count: number;
  cleanup_succeeded: boolean;
};
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `memory_peak_bytes` | `number` | Peak memory usage (bytes) |
| `cpu_usage_usec` | `number` | CPU usage (microseconds) |
| `cpu_nr_throttled` | `number` | Number of `cpu.max` throttling periods |
| `cpu_throttled_usec` | `number` | Total throttled time from `cpu.stat` |
| `pids_peak` | `number` | Peak process count |
| `execution_time_ms` | `number` | Execution time (milliseconds) |
| `stdout_truncated` | `boolean` | Whether stdout exceeded its configured byte limit |
| `stderr_truncated` | `boolean` | Whether stderr exceeded its configured byte limit |
| `outcome` | `string` | Explicit terminal outcome derived from wait status and cgroup events |
| `termination_signal` | `number \| null` | Signal number when the task was killed by a signal |
| `oom_kill_count` | `number` | `memory.events` OOM-kill count for this task cgroup |
| `pids_limit_hit_count` | `number` | `pids.events` maximum-hit count for this task cgroup |
| `cleanup_succeeded` | `boolean` | Whether process-tree termination and cgroup removal succeeded |

## TaskGroupResult

Result of executing a task group.

```typescript
type TaskGroupResult = (TaskResult | TaskResult[])[];
```

Each element corresponds to an execution step:
- **Single task step** → `TaskResult`
- **Parallel tasks step** → `TaskResult[]`

### Example

```json
[
  {
    "stdout": "Step 1\n",
    "stderr": "",
    "exit_code": 0,
    "stats": { "memory_peak_bytes": 1048576, "cpu_usage_usec": 1000, "pids_peak": 1, "execution_time_ms": 5 }
  },
  [
    {
      "stdout": "Parallel A\n",
      "stderr": "",
      "exit_code": 0,
      "stats": { "memory_peak_bytes": 1048576, "cpu_usage_usec": 500, "pids_peak": 1, "execution_time_ms": 3 }
    },
    {
      "stdout": "Parallel B\n",
      "stderr": "",
      "exit_code": 0,
      "stats": { "memory_peak_bytes": 1048576, "cpu_usage_usec": 600, "pids_peak": 1, "execution_time_ms": 4 }
    }
  ]
]
```

## HealthResponse

Health check response.

```typescript
type HealthResponse = {
  status: 'ok';
};
```

## ErrorResponse

Error response format.

```typescript
type ErrorResponse = {
  error: string;
  message: string;
};
```

## TypeScript SDK Types

The JavaScript/TypeScript SDK uses camelCase variants:

### TaskResult (SDK)

```typescript
type TaskResult = {
  stdout: string;
  stderr: string;
  exitCode: number;  // Note: camelCase
  stats?: ExecutionStats;
};
```

The SDK automatically converts between snake_case (API) and camelCase (SDK).

## Validation Rules

### Task Validation

- `cmd` must not be empty
- `args` must be an array of strings (if provided)
- `env` keys must be valid environment variable names
- `files` paths must not contain `..` or start with `/`

### ExecutionStep Validation

- Must be a valid Task object or array of Task objects
- Empty arrays are not allowed

### TaskGroup Validation

- Must be a non-empty array
- Each element must be a valid ExecutionStep

## JSON Schema

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "Task": {
      "type": "object",
      "required": ["cmd"],
      "properties": {
        "cmd": { "type": "string" },
        "args": { "type": "array", "items": { "type": "string" } },
        "env": { "type": "object", "additionalProperties": { "type": "string" } },
        "stdin": { "type": "string" },
        "files": { "type": "object", "additionalProperties": { "type": "string" } },
        "working_dir": { "type": "string" },
        "sandbox_profile": { "enum": ["compile_v1", "native_v1"] }
      }
    },
    "TaskResult": {
      "type": "object",
      "required": ["stdout", "stderr", "exit_code"],
      "properties": {
        "stdout": { "type": "string" },
        "stderr": { "type": "string" },
        "exit_code": { "type": "integer" },
        "stats": {
          "type": "object",
          "properties": {
            "memory_peak_bytes": { "type": "integer" },
            "cpu_usage_usec": { "type": "integer" },
            "cpu_nr_throttled": { "type": "integer" },
            "cpu_throttled_usec": { "type": "integer" },
            "pids_peak": { "type": "integer" },
            "execution_time_ms": { "type": "integer" },
            "stdout_truncated": { "type": "boolean" },
            "stderr_truncated": { "type": "boolean" },
            "outcome": { "type": "string" },
            "termination_signal": { "type": ["integer", "null"] },
            "oom_kill_count": { "type": "integer" },
            "pids_limit_hit_count": { "type": "integer" },
            "cleanup_succeeded": { "type": "boolean" }
          }
        }
      }
    }
  }
}
```

## See Also

- [REST API Endpoints](/api/rest/endpoints/) - API endpoint documentation
- [JavaScript SDK](/sdk/javascript/overview/) - SDK type definitions
