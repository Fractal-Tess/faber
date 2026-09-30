# Faber security model and verification

The tested hardware, attack families, and explicit exclusions are recorded in
[`SECURITY_TEST_MATRIX.md`](SECURITY_TEST_MATRIX.md).

Faber's namespace backend runs untrusted processes in Linux namespaces and
cgroup v2. It shares the outer host kernel, so it is not equivalent to a
microVM and cannot contain a host-kernel vulnerability. Faber is not yet ready
for public hostile workloads; the remaining release gates are tracked in
[`ROADMAP.md`](ROADMAP.md).

## Threat model

An attacker controls task commands, arguments, environment variables, stdin,
submitted file names and contents, working directories, source code, and
executed binaries. They may attempt path traversal, symlink races, process and
memory exhaustion, output flooding, namespace escape, cross-task observation,
and persistence after task completion.

The host kernel, outer Docker image, Faber daemon, cgroup hierarchy, configured
toolchains, and operator are trusted. The namespace backend does not claim to
mitigate kernel vulnerabilities or every microarchitectural side channel.

## Isolation invariants

An invariant is considered verified only when it has both an implementation
review and an executable acceptance test. A smoke test or the presence of a
namespace flag is not sufficient evidence.

| Area | Required invariant | Evidence | Status |
|---|---|---|---|
| Workspace files | Submitted paths are normalized regular files beneath `/faber`; symlinks, magic links, cross-mount hard links, directories, FIFOs, sockets, devices, and swap races cannot redirect writes | `openat2` resolution plus object-type, nonblocking FIFO, and parallel symlink-swap tests | Verified baseline |
| Root filesystem | The old root is detached, propagation is private, toolchains are read-only, writable tmpfs mounts are `nodev,nosuid`, and task sysfs exposes no cgroup mount | Filesystem boundary, device-node, and mountinfo assertions | Verified baseline |
| PID/proc | Host processes are absent, PID 1 cannot be signaled or traversed through `/proc/1/root`, and descendants are reaped | Privilege/proc probe, orphan reaping, full-cgroup timeout termination, and cleanup tests | Verified baseline |
| Network | Tasks have no host or external connectivity over IPv4 or IPv6, no resolver configuration, and independent runtimes never reuse a network namespace | Interface, route-table, IPv4/IPv6 nonblocking-connect, DNS visibility, native socket-policy, and namespace-uniqueness tests | Verified baseline |
| User identity | Each task has a fresh user namespace mapping only inner 65534:65534 to outer 65534:65534; supplementary groups are empty | Controller compares namespace inodes and verifies exact one-entry UID/GID maps from the probe | Verified baseline |
| Privileges | Capability and identity regain, namespace-map rewriting, chroot, hostname changes, and device access fail; no setup FDs survive `exec` | Kernel-state and active privilege-escape probes | Verified baseline |
| Syscalls | Every task installs a versioned seccomp policy before `exec`; violations terminate with `SIGSYS` | Probe verifies mode 2; the matrix test invokes every blocked syscall under each applicable profile and verifies `policy_violation` | Verified denylist baseline |
| Memory | The complete task process tree cannot exceed `memory.max`; a kill caused by the task's own limit (`out_of_memory`) is reported separately from one caused by an ancestor limit (`ancestor_out_of_memory`) | OOM acceptance test, ancestor-limit test, and reported `memory.events:oom_kill` / `memory.events.local:oom` evidence | Verified baseline |
| Request fan-out | Requests cannot exceed the configured step or parallel-task counts. `MAX_CONCURRENCY` counts task slots and a request reserves one per task of its widest step, so the service cgroup's aggregate (`(memory.max + workspace tmpfs) × slots`, `pids.max × slots`) covers every admitted task at its own limit. Each request cgroup is capped at its widest step times the per-task limits plus its workspace, so one request's pressure cannot pick a victim in another | API rejection and slot-reservation tests, request-cgroup sizing test, startup cgroup configuration | Verified baseline |
| Syscall policy | Service-configured profiles deny namespace creation, modern mount APIs, privileged handles, and x32 syscall-number bypasses | Per-syscall policy probes, clone namespace probe, clone3 fallback probe, and x32 probe | Verified denylist baseline |
| Process count | The complete task process tree cannot exceed `pids.max` | PID acceptance test and reported `pids.events:max` evidence | Verified baseline |
| CPU | CPU bandwidth, CPU time, and wall time are independently bounded | Busy-loop test verifies `cpu.max` throttling counters and a shorter `RLIMIT_CPU` terminates before wall timeout | Verified baseline |
| Rlimits | CPU time, file size, descriptors, stack, and core dumps have finite enforced policy limits | Probe verifies configured values; active tests hit `EMFILE`, `EFBIG`, stack/core signals, absent core files, and CPU kill | Verified baseline |
| Output | stdin/stdout/stderr progress concurrently, each output stream is bounded, and the request as a whole keeps at most `REQUEST_OUTPUT_LIMIT_BYTES` of output (so every in-memory copy of a result is bounded) | Flood and bidirectional-pipe tests report `output_limit` and truncation; request-budget test shows later tasks receive the remainder and then nothing | Verified baseline |
| Request scoping | Each execution owns a `faber/req-<id>` cgroup containing all of its task cgroups; a request's overall-deadline kill is confined to that subtree and cannot terminate another request's tasks | Concurrent-request deadline test asserts the unrelated request exits normally | Verified baseline |
| Admission | A client disconnect neither frees its concurrency slot nor leaves its sandbox running: the slot is held until the runtime returns and the disconnect cancels the request cgroup | Disconnecting-client test bounds live sandboxes by the limit; abort test requires teardown well before the wall timeout | Verified baseline |
| Cleanup | No process, request or task cgroup, or container root survives success, task failure, timeout, output kill, policy violation, API cancellation, service shutdown, or partial container setup; shutdown stops later steps of running requests and bounds the drain | Outcome tests, request cancellation test, shutdown test, setup-failure root comparison, and post-run host assertions | Verified baseline |

“Verified baseline” describes the behavior covered by the current test and is
not a claim that the whole isolation area is complete. Tests must be expanded
for races, concurrency, cancellation, and kernel-version differences.

## Probe baseline

`tests/fixtures/security_probe.c` records kernel state as JSON from inside the
untrusted process after security setup. The initial privileged-Docker baseline
confirmed the expected gaps: supplementary group 0 remained, `CapBnd` was
nonzero, `NoNewPrivs` and seccomp mode were both 0, UID/GID maps still covered
the initial user namespace, CPU/file/core limits were unlimited, and sysfs was
writable inside the mount namespace. Privilege cleanup now produces empty
supplementary and capability sets with `NoNewPrivs: 1`; mount hardening now
provides private propagation and read-only sysfs without the cgroup mount;
rlimits bound CPU/file/FD/stack/core resources; and each task now receives an
explicit one-identity user namespace. Versioned compile/native seccomp
denylists now trap known high-risk interfaces; exhaustive allowlists remain
future hardening.

## Running verification

Never execute Faber or its privileged runtime tests directly on the NixOS host.
Run the focused suite through the rootful privileged development container:

```bash
./scripts/dev.sh test-security
```

Run the complete Rust suite with:

```bash
./scripts/dev.sh test
```

The test harness is single-threaded because the runtime currently forks and
performs setup before `exec`; concurrent execution receives a separate stress
test rather than relying on the Rust test harness's scheduling.

Destructive abuse tests, mount propagation probes, fork/OOM enforcement, and
concurrent-cgroup tests run in `quality-and-security.yml` on disposable GitHub
hosted VMs. Future escape-oriented tests belong there as well. Do not run kernel
exploits as sandbox tests.

## Kernel evidence to capture

Future host-observer tests should pause a task after cgroup attachment and
record:

- namespace inode IDs from `/proc/<pid>/ns/*`
- UID/GID maps, supplementary groups, capability sets, `NoNewPrivs`, and
  seccomp mode from `/proc/<pid>/status`
- `/proc/<pid>/mountinfo` and the task's visible procfs
- task cgroup membership and effective `cpu.max`, `memory.max`, and `pids.max`
- `cpu.stat`, `memory.events`, `memory.peak`, `pids.events`, and `pids.peak`
- `cgroup.events` before cleanup and the absence of task cgroups afterward

## Reference material

- [namespaces(7)](https://man7.org/linux/man-pages/man7/namespaces.7.html)
- [mount_namespaces(7)](https://man7.org/linux/man-pages/man7/mount_namespaces.7.html)
- [pid_namespaces(7)](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html)
- [user_namespaces(7)](https://man7.org/linux/man-pages/man7/user_namespaces.7.html)
- [capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [openat2(2)](https://man7.org/linux/man-pages/man2/openat2.2.html)
- [seccomp(2)](https://man7.org/linux/man-pages/man2/seccomp.2.html)
- [fork(2)](https://man7.org/linux/man-pages/man2/fork.2.html)
- [Linux cgroup v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [nsjail](https://github.com/google/nsjail)
- [isolate](https://github.com/ioi/isolate)

## Reporting vulnerabilities

Do not publish suspected vulnerabilities in a public issue. Contact the
maintainer privately with the affected revision, reproduction steps, expected
isolation invariant, observed kernel evidence, and potential impact.
