# Faber HTTP API

Every route lives under `/api/v1`. All routes except `/health` need the API
key, as `Authorization: Bearer <key>` or the raw key. Every response carries
an `X-Request-Id` header that also appears in the request log line.

## Health

```bash
GET /api/v1/health
```

Returns `{"status":"ok","version":"..."}` without authentication. Once
shutdown has begun it answers `503` with `"status":"shutting_down"`, so a load
balancer stops routing to an instance that is draining.

## Metrics

```bash
GET /api/v1/metrics   # Authorization: Bearer <key>
```

Prometheus text format: `/execute` responses by status, tasks by outcome,
request duration, task slots configured and in use, and result-cache hits and
entries.

## Execute tasks

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
- `sandbox_profile` (optional): seccomp profile, one of those in
  `ALLOWED_SANDBOX_PROFILES` (see [INSTALL.md](INSTALL.md#execution-limits))

**Response:** an array of task results with `stdout`, `stderr`, `exit_code`
and `stats`. `stats.outcome` says how the task ended: `exited`, `signaled`,
`timed_out`, `out_of_memory`, `ancestor_out_of_memory`, `pids_limit`,
`output_limit`, `policy_violation`, `not_started` or
`infrastructure_failure`. The other stats are resource use: CPU time, peak
memory, peak processes, wall time and whether output was truncated.

## Stored files

The `/file` endpoints are a content-addressed artifact store. Stored file IDs are
not currently accepted by `/execute`; use a task's inline `files` map to
materialize inputs in its workspace. Connecting store objects to executions
requires an explicit task-reference and authorization design and is intentionally
not implied by uploading an object.

The store is bounded: uploads beyond `FABER_STORE_MAX_TOTAL_BYTES` or
`FABER_STORE_MAX_ENTRIES` are rejected with `507 Insufficient Storage`, a file
above `UPLOAD_FILE_LIMIT_BYTES` with `413`, and more than
`MAX_CONCURRENT_UPLOADS` simultaneous uploads with `503`. Files expire
`FABER_STORE_TTL_SECS` after they were last uploaded or read; expired files are
treated as absent on read and removed by a sweep every
`FABER_STORE_TTL_CHECK_SECS`.

## Examples

Compile and run C. Steps run in order in one sandbox, so the second step sees
the binary the first produced:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "/usr/bin/gcc",
      "args": ["hello.c", "-o", "hello"],
      "files": {
        "hello.c": "#include <stdio.h>\nint main() { printf(\"Hello!\\n\"); return 0; }"
      }
    },
    {"cmd": "./hello"}
  ]'
```

An array inside the request is a parallel step:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '[
    {"cmd": "echo", "args": ["Task 1"]},
    [
      {"cmd": "echo", "args": ["Parallel A"]},
      {"cmd": "echo", "args": ["Parallel B"]}
    ]
  ]'
```

The response mirrors the request: one result per task, nested the same way.
