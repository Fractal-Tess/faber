# Faber

<div align="center">

![Faber Logo](faber.png)

**Secure, isolated task execution runtime built in Rust**

[![Build and Push Docker Image](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml/badge.svg?branch=main)](https://github.com/Fractal-Tess/faber/actions/workflows/docker-build-push.yml)

</div>

---

Faber is an experimental task execution runtime that runs commands in Linux namespaces with cgroup v2 resource controls. The namespace backend is under active hardening and is not yet suitable for public hostile workloads; see [`SECURITY.md`](SECURITY.md), [`SECURITY_TEST_MATRIX.md`](SECURITY_TEST_MATRIX.md), and [`ROADMAP.md`](ROADMAP.md).

## ✨ Features

- 🔒 **Namespace Isolation** - Linux mount, PID, network, UTS, and IPC namespaces
- 📊 **Resource Monitoring** - Real-time tracking of CPU, memory, and process usage
- ⚡ **Parallel Execution** - Run multiple tasks concurrently within isolated namespaces
- 🎯 **Flexible API** - RESTful API with support for sequential and parallel task groups
- 🚀 **High Performance** - Built with Rust for minimal overhead
- 📦 **Docker Ready** - Easy deployment with containerized builds

## 🚀 Quick Start

### Prerequisites

- Docker (with privileged mode support)
- Linux kernel with cgroups v2 support

### Running Faber

1. **Start the service.** The published image contains GCC, G++ and Make. An
   API key is required; generate one with `openssl rand -hex 32`.

```bash
docker run -d --name faber --init \
  --privileged --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw \
  --read-only --tmpfs /tmp:size=64m \
  --memory 8g --memory-swap 8g --pids-limit 4096 \
  -e API_KEY="$API_KEY" \
  -p 127.0.0.1:3000:3000 \
  vgfractal/faber:latest
```

   `docker/prod/docker-compose.yaml` is the same deployment as a Compose file.
   Put a TLS-terminating reverse proxy in front before exposing it beyond
   localhost; see [OPERATIONS.md](OPERATIONS.md).

2. **Add toolchains** by extending the image. Anything under `/usr`, `/bin`
   and `/lib` is visible to tasks read-only
   (`docker/toolchains/Dockerfile` adds Python, Node.js, Java, Go and Rust):

```dockerfile
FROM vgfractal/faber:latest
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
```

3. **Execute a task**:

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '[
    {
      "cmd": "echo",
      "args": ["Hello, Faber!"]
    }
  ]'
```

---

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

Returns `{"status":"ok","version":"..."}` without authentication. Once
shutdown has begun it answers `503` with `"status":"shutting_down"`, so a load
balancer stops routing to an instance that is draining.

### Metrics

```bash
GET /api/v1/metrics   # Authorization: Bearer <key>
```

Prometheus text format: `/execute` responses by status, tasks by outcome,
request duration, task slots configured and in use, and result-cache hits and
entries. Every response carries an `X-Request-Id` header that also appears in
the request log line.

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

### Stored files and task files

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

## ⚙️ Configuration

Faber reads its settings from environment variables and refuses to start if one
is malformed or out of range; the error names the variable.

### Service

| Variable | Default | Meaning |
|---|---|---|
| `API_KEY` | required | Key expected in the `Authorization` header (`Bearer <key>` or the raw key). A comma-separated list accepts every listed key, so a key can be rotated without downtime |
| `HOST` | `0.0.0.0` | Listen address |
| `PORT` | `3000` | Listen port |
| `CACHE_ENABLED` | `false` | Whole-request memoization (`true`/`false`/`1`/`0`). Only for deterministic workloads: an identical request is answered from memory. Only requests whose every task exited 0 with complete output are remembered |
| `CACHE_MAX_ENTRIES` | `1024` | Results held by the cache; the oldest is evicted first |
| `CACHE_MAX_BYTES` | `67108864` | Output bytes held across all cached results |
| `CACHE_TTL_SECS` | `300` | Age after which a cached result is discarded |
| `LOG_FORMAT` | text | `json` writes one JSON object per log line |
| `RUST_LOG` | `faber=info,faber_api=info,faber_runtime=info` | Log filter |
| `SHUTDOWN_TIMEOUT_MS` | `5000` | How long in-flight requests may drain after SIGTERM; keep it below the container stop grace period (10 s for Docker) |

### Execution limits

| Variable | Default | Meaning |
|---|---|---|
| `MAX_CONCURRENCY` | `10` | Task slots: sandboxed tasks that may run at once across all requests. A request reserves one slot per task of its widest step and holds them until its sandbox has finished; requests that do not fit get `503` |
| `MAX_PARALLEL_TASKS` | `8` | Tasks in one parallel step; must not exceed `MAX_CONCURRENCY` |
| `MAX_STEPS_PER_REQUEST` | `64` | Steps in one request |
| `MEMORY_MAX` | `256M` | Per-task `memory.max` (bytes or `K`/`M`/`G`/`T`); must be finite and at least `1M` |
| `PIDS_MAX` | `64` | Per-task `pids.max` |
| `CPU_MAX` | `50000 100000` | Per-task `cpu.max`: `<quota> [<period>]`, quota `max` or at least 1000 µs, period 1000–1000000 µs |
| `WALL_TIMEOUT_MS` | `5000` | Wall-clock limit per task |
| `CPU_TIME_LIMIT_SECS` | `5` | `RLIMIT_CPU` per task process |
| `OVERALL_TIMEOUT_MS` | `30000` | Deadline for a whole request (see below) |
| `OUTPUT_LIMIT_BYTES` | `1048576` | Bytes kept per output stream per task |
| `REQUEST_OUTPUT_LIMIT_BYTES` | `16777216` | Output bytes kept across every task of a request |
| `EXECUTE_BODY_LIMIT_BYTES` | `1048576` | Maximum `/execute` request body |
| `DEFAULT_SANDBOX_PROFILE` | `compile_v2` | Seccomp profile for tasks that do not name one |
| `ALLOWED_SANDBOX_PROFILES` | `compile_v2,native_v2` | Profiles a request may select. `v2` profiles are allowlists (unlisted syscalls fail with `ENOSYS`); `v1` are the older denylists |
| `SANDBOX_READONLY_PATHS` | `/bin,/lib,/lib64,/usr,/etc/alternatives,/etc/ld.so.cache,/etc/passwd,/etc/group` | Paths of the image that tasks see read-only. Replaces the default list; add what a toolchain keeps elsewhere, such as `/etc/java-17-openjdk` |
| `SANDBOX_IDENTITY_BASE` | `100000` | First host UID/GID leased to requests |
| `SANDBOX_IDENTITY_COUNT` | `65536` | Size of that range; give each Faber service on a kernel its own |

### File store

| Variable | Default | Meaning |
|---|---|---|
| `FABER_STORE_BACKEND` | `memory` | `memory`, `filesystem` or `hybrid` |
| `FABER_STORE_PATH` | `/var/lib/faber/store` | Directory for the `filesystem` and `hybrid` backends |
| `FABER_STORE_MAX_TOTAL_BYTES` | `536870912` | Total bytes of stored files; uploads beyond it get `507` |
| `FABER_STORE_MAX_ENTRIES` | `1000` | Number of stored files; uploads beyond it get `507` |
| `FABER_STORE_TTL_SECS` | `3600` | Files expire this long after their last upload or read; `0` disables expiry |
| `FABER_STORE_TTL_CHECK_SECS` | `60` | Interval of the sweep that removes expired files |
| `FABER_STORE_MAX_MEMORY_ENTRIES` | `1000` | `hybrid` only: files cached in memory |
| `FABER_STORE_MAX_MEMORY_SIZE` | `104857600` | `hybrid` only: bytes cached in memory |
| `UPLOAD_FILE_LIMIT_BYTES` | `52428800` | Maximum size of one uploaded file (`413` above it) |
| `MAX_CONCURRENT_UPLOADS` | `4` | Uploads buffered at once (`503` above it) |

### How the limits relate

- **Aggregate resources.** The `faber` cgroup is capped at
  `(MEMORY_MAX + supervisor allowance) × MAX_CONCURRENCY` bytes and
  `(PIDS_MAX + 3) × MAX_CONCURRENCY` processes, which covers every admitted
  task at its own limit together with the root processes that run it. The
  supervisor allowance is `64 MiB + 2 × REQUEST_OUTPUT_LIMIT_BYTES` (the
  output capped at 1 GiB) plus the 256 MiB of workspace and `/tmp` tmpfs.
  Each request runs in its own cgroup capped at its widest step times the
  per-task limits plus that allowance; its jailer and supervisors sit in a
  `supervisor` leaf of it, capped at the allowance and marked as the OOM
  killer's last choice.
- **Deadlines.** Each step's wall timeout is clipped to what is left of
  `OVERALL_TIMEOUT_MS`, and steps that cannot start are returned with the
  `not_started` outcome alongside every completed result. With the defaults a
  request of 64 slow steps (64 × 5 s) cannot finish within 30 s; Faber logs a
  warning at startup when `MAX_STEPS_PER_REQUEST × WALL_TIMEOUT_MS` exceeds
  `OVERALL_TIMEOUT_MS`.
- **Memory per request.** A response holds at most
  `REQUEST_OUTPUT_LIMIT_BYTES` of task output. JSON escaping can grow control
  bytes up to six times, and the result exists in the sandbox controller, the
  API process and (with `CACHE_ENABLED`) the cache, so budget roughly
  `6 × REQUEST_OUTPUT_LIMIT_BYTES` per copy per concurrent request.

## 🏗️ Architecture

Faber consists of three main components:

- **`faber-runtime`** - Core runtime with container isolation, cgroups, and resource monitoring
- **`faber-api`** - HTTP API server with opt-in request memoization and task orchestration
- **SDKs** - Client libraries for various languages (JavaScript/TypeScript available)

## 📚 Documentation

- [OPERATIONS.md](OPERATIONS.md) - deployment, sizing, monitoring and runbook
- [SECURITY.md](SECURITY.md) - threat model, isolation invariants and how they are verified
- [DEVELOPMENT.md](DEVELOPMENT.md) - development loop and the disposable test VM
- [ROADMAP.md](ROADMAP.md) - what is planned and what is deliberately out of scope
- [docs/](docs/) - the documentation site (API types, SDK, concepts)

## 🛠️ Development

Faber's runtime must be developed and tested inside Docker. In particular, do
not use `cargo run` on NixOS: the runtime needs a conventional FHS filesystem,
privileged namespace operations, and writable cgroup v2 access.

See [DEVELOPMENT.md](DEVELOPMENT.md) for the complete workflow. The short path
is:

```bash
./scripts/dev.sh up
./scripts/dev.sh logs
./scripts/dev.sh test
./scripts/dev.sh down
```

The security and caching work required before exposing Faber to hostile users
is tracked in [ROADMAP.md](ROADMAP.md).

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

Faber runs every request in a namespace sandbox on the host kernel:

- **Namespaces** - mount, network, UTS and IPC per request; PID and user per
  task. Tasks have no network and see a per-process `/proc` and an empty `/sys`
- **Identity** - each request runs as its own unprivileged host UID with no
  capabilities, behind a re-executed jailer that holds none of the service's
  memory or environment
- **Seccomp** - allowlist profiles by default; violations kill the process
- **Cgroups v2** - CPU, memory and process limits per task, per request and
  for the whole service, plus rlimits and bounded output
- **Authentication** - every route except `/health` requires the API key

**Isolation tier.** This is kernel-shared isolation, the same class as a
hardened container. It contains ordinary hostile programs and bounds their
resource use; it does not protect against a Linux kernel vulnerability. For
anonymous public workloads, run Faber inside a dedicated VM per trust domain.
The threat model, verified invariants and residual risks are in
[SECURITY.md](SECURITY.md); deployment and incident handling are in
[OPERATIONS.md](OPERATIONS.md).

## 📊 Status

Tested on every push: the sandbox acceptance and stress suites on x86_64 and
ARM64, a production-image smoke test, and C, C++, Python, Node.js, Java, Go
and Rust toolchains under the default profile (x86_64). The suites also pass
on Linux 6.6, 6.12 and 6.16; 5.8 is the minimum.

Not included: a gVisor or microVM backend, per-tenant keys and quotas, use of
stored files as task inputs, and compile-artifact caching. See
[ROADMAP.md](ROADMAP.md).

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

**Built with ❤️ using Rust**
