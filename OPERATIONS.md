# Operating Faber

How to deploy Faber, what to watch, and what to do when something goes wrong.
The security model is in [SECURITY.md](SECURITY.md); every setting is listed
in the [README](README.md#️-configuration).

## What you are deploying

Faber runs untrusted commands in namespace sandboxes on the kernel of the
machine it runs on. Its container is privileged and sees the host cgroup tree,
so **the Faber container is root-equivalent on its host**. Plan for that:

- Give Faber a machine (or VM) of its own. Do not co-locate it with data or
  services that the sandboxed code must never reach.
- A kernel vulnerability reachable from the sandbox is a host compromise.
  Keep the host kernel patched. For anonymous public workloads, one VM per
  trust domain is the boundary, not Faber.
- There is one API key scope: every key holder can run code and read every
  stored file. Faber does not separate tenants.

## Deployment

Use `docker/prod/docker-compose.yaml`, or the equivalent `docker run` from the
README. The parts that matter:

| Setting | Why |
|---|---|
| `privileged`, `cgroup: host`, `/sys/fs/cgroup` read-write | Required to create namespaces and cgroups. Faber refuses to start if the cgroup namespace and mount disagree |
| `init: true` | Reaps supervisor processes orphaned by an abnormal exit |
| `read_only` with a `/tmp` tmpfs | Faber writes only sandbox root skeletons there |
| `mem_limit`, `memswap_limit`, `pids_limit` | Backstop above Faber's own aggregate limits (see sizing) |
| Port bound to `127.0.0.1` | Faber speaks plain HTTP. Terminate TLS in a reverse proxy and forward only `/api/v1/` |
| `FABER_IMAGE` pinned to a version tag | `latest` moves with every push to `main` |

Requirements: Linux 5.8 or newer with cgroup v2, rootful Docker. Tested on
x86_64 and ARM64.

**API keys.** Generate with `openssl rand -hex 32`. `API_KEY` accepts a
comma-separated list; to rotate, deploy `old,new`, move clients to the new
key, then deploy `new` alone.

**Toolchains.** Extend the image (`docker/toolchains/Dockerfile` is an
example). Tasks see `/usr`, `/bin`, `/lib` and a few files under `/etc`
read-only; add other paths with `SANDBOX_READONLY_PATHS`. Run
`scripts/test-toolchains.sh` against a new image before relying on it: the
default seccomp profile is an allowlist, and an unlisted syscall fails with
`ENOSYS`.

## Sizing

Faber admits at most `MAX_CONCURRENCY` tasks at once and sheds the rest with
`503` and `Retry-After`. Memory it can use:

```
(MEMORY_MAX + 64 MiB + 2 x REQUEST_OUTPUT_LIMIT_BYTES + 256 MiB) x MAX_CONCURRENCY
```

With the defaults that is about 6.3 GiB and 670 processes. Set the container
`mem_limit` above that figure plus roughly 256 MiB for the service itself, and
give the host that much free memory. CPU is `CPU_MAX` (half a core by default)
per task.

## Monitoring

`GET /api/v1/metrics` (with the API key) serves Prometheus metrics;
`GET /api/v1/health` needs no key.

| Signal | Meaning | Act when |
|---|---|---|
| `faber_execute_responses_total{status="503"}` | Requests shed for lack of slots | Sustained growth: raise `MAX_CONCURRENCY` (and memory) or add instances |
| `faber_execute_responses_total{status="500"}` | Sandbox or runtime failure | Any occurrence: read the log line with the same request ID |
| `faber_execute_responses_total{status="504"}` | A request hit the hard overall deadline | Repeated: the host is overloaded or a jailer is stuck |
| `faber_tasks_total{outcome="infrastructure_failure"}` | A task could not be set up | Any occurrence |
| `faber_tasks_total{outcome="ancestor_out_of_memory"}` | A task was killed by a limit above its own | Growth: memory is undersized for `MAX_CONCURRENCY` |
| `faber_tasks_total{outcome="policy_violation"}` | A task made a denied syscall | Spikes: someone is probing, or a toolchain needs a syscall the profile denies |
| `faber_execution_slots_in_use` | Slots held now | Stuck at the maximum with no traffic: see leaked sandboxes |
| `/health` returns 503 | The instance is draining | Expected during shutdown only |

Logs go to stdout; `LOG_FORMAT=json` for collectors. Every request line has a
`request_id` that is also returned as `X-Request-Id`.

## Runbook

**Requests are shed (503) although traffic is low.** Check
`faber_execution_slots_in_use`. A slot is held until its sandbox is really
gone, so abandoned requests release theirs within the wall timeout. If slots
stay held, restart the container: startup kills every leftover sandbox and
removes its cgroups and root directories.

**Leaked cgroups or sandbox roots.** Inside the container, request cgroups
are `faber/req-*` beneath the container's own cgroup, and roots are
`/tmp/faber/*`. They should exist only while a request runs. Leftovers after a
crash are reclaimed at the next start. To inspect:

```bash
docker exec faber-prod sh -c 'find /sys/fs/cgroup -type d -name "req-*"; ls /tmp/faber'
```

**OOM kills.** `out_of_memory` is a task exceeding `MEMORY_MAX`: expected for
hostile or oversized workloads. `ancestor_out_of_memory` means a request,
service or container limit was hit first: lower `MAX_CONCURRENCY` or raise the
container memory limit. If the kernel log shows the Faber process itself being
killed, the container limit is below the sizing formula.

**Policy violations from a legitimate toolchain.** The task dies with
`policy_violation` (a denied syscall) or misbehaves after an `ENOSYS` (an
unlisted one). Reproduce with `strace -f` outside Faber to find the syscall.
Denied syscalls are denied on purpose. For an unlisted one, the `v1` profiles
(`ALLOWED_SANDBOX_PROFILES=compile_v1,native_v1`) are denylists and will run
it; extend `ALLOWED_SYSCALLS` in `crates/faber-runtime/src/runtime/core.rs`
for a permanent fix.

**Slow or hanging requests.** Every task has a wall timeout and every request
an overall deadline with a hard backstop, so a request cannot outlive
`OVERALL_TIMEOUT_MS` by more than a few seconds. If requests are slow across
the board, check host CPU and memory pressure and `cpu_throttled_usec` in the
task statistics.

**Shutdown and upgrades.** `SIGTERM` stops new work, kills running sandboxes
and exits within `SHUTDOWN_TIMEOUT_MS`; `/health` returns 503 meanwhile.
In-flight requests receive `503`. To upgrade without failed requests, take the
instance out of the load balancer first, wait for
`faber_execution_slots_in_use` to reach 0, then replace it.

**Suspected sandbox escape or host compromise.** Treat the host as
compromised: the container is privileged. Stop the container, take the host
out of service, preserve the logs (request IDs identify the submitting
requests; request bodies are not logged), rotate the API keys, and rebuild
the host. Report the finding as described in [SECURITY.md](SECURITY.md).

## Verifying a deployment

```bash
./scripts/vm.sh prepare          # once: builds the images in a disposable VM
./scripts/vm.sh test             # sandbox acceptance suites
./scripts/vm.sh test-docker      # production image smoke test
./scripts/vm.sh test-toolchains  # language toolchains under the default profile
./scripts/vm.sh soak             # mixed concurrent load, then leak checks
```

`scripts/soak.sh` and `scripts/toolchain-test.sh` also run against any
reachable instance (`API_URL`, `API_KEY`). The soak includes hostile requests
and expects the documented default limits; run it against staging, not
production.
