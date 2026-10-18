# Installing Faber

Faber ships as a Docker image. This page covers running it, adding language
toolchains, and every setting it reads. Sizing, monitoring and the incident
runbook are in [OPERATIONS.md](OPERATIONS.md); the threat model is in
[SECURITY.md](SECURITY.md).

## Requirements

- Linux 5.8 or newer with cgroup v2 (x86_64 or ARM64)
- Rootful Docker with privileged containers. Rootless Docker cannot grant the
  mount capabilities Faber needs for nested PID and mount namespaces.

## Run the published image

The published image contains GCC, G++ and Make. An API key is required;
generate one with `openssl rand -hex 32`.

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
Faber speaks plain HTTP: put a TLS-terminating reverse proxy in front before
exposing it beyond localhost (see [OPERATIONS.md](OPERATIONS.md)).

Check that it answers:

```bash
curl http://localhost:3000/api/v1/health
curl -X POST http://localhost:3000/api/v1/execute \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '[{"cmd": "/bin/echo", "args": ["hello"]}]'
```

## Add toolchains

Extend the image. Anything under `/usr`, `/bin` and `/lib` is visible to tasks
read-only; `docker/toolchains/Dockerfile` adds Python, Node.js, Java, Go and
Rust.

```dockerfile
FROM vgfractal/faber:latest
RUN apt-get update && apt-get install -y --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*
```

Toolchains that keep files elsewhere (the JDK under `/etc/java-17-openjdk`)
need those paths added to `SANDBOX_READONLY_PATHS`. Run
`scripts/test-toolchains.sh` against a new image before relying on it: the
default seccomp profile is an allowlist, and an unlisted syscall fails with
`ENOSYS`.

## Configuration

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

