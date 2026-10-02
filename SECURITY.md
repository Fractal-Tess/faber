# Faber security model and verification

The tested hardware, attack families, and explicit exclusions are recorded in
[`SECURITY_TEST_MATRIX.md`](SECURITY_TEST_MATRIX.md).

Faber's namespace backend runs untrusted processes in Linux namespaces and
cgroup v2. It shares the outer host kernel, so it is not equivalent to a
microVM and cannot contain a host-kernel vulnerability.

**Isolation tier.** Kernel-shared isolation, the class of a hardened
container: suitable for running untrusted code from authenticated users on a
host dedicated to Faber. It is not, on its own, a boundary for anonymous
public workloads or between mutually hostile tenants; put each trust domain in
its own VM for that. The Faber container is privileged, so a sandbox escape is
a compromise of its host. Deployment guidance is in
[`OPERATIONS.md`](OPERATIONS.md); stronger backends are tracked in
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
| Workspace files | Submitted paths are normalized regular files beneath `/faber`; symlinks, magic links, cross-mount hard links, directories, FIFOs, sockets, devices, and swap races cannot redirect writes or directory creation. Parent directories are created one component at a time relative to the verified parent descriptor, and created directories and files belong to the task user | Per-component `mkdirat` + `openat2` resolution plus object-type, nonblocking FIFO, parallel file symlink-swap, and `renameat2(RENAME_EXCHANGE)` nested-directory swap tests; nested-directory writability test | Verified baseline |
| Root filesystem | The old root is detached, propagation is private, toolchains are read-only, writable tmpfs mounts are `nodev,nosuid`, and `/sys` is an empty read-only tmpfs (no sysfs, no cgroup mount) | Filesystem boundary, device-node, and mountinfo assertions | Verified baseline |
| PID/proc | Every task has a PID namespace of its own, so host processes and the other tasks of a parallel step are absent and cannot be signaled; PID 1 cannot be signaled or traversed through `/proc/1/root`, and descendants are reaped. procfs is mounted with `subset=pid`, so host-wide files (`/proc/cmdline`, `meminfo`, `stat`, `loadavg`, `interrupts`, `/proc/sys`) do not exist; tools that need them, such as `ps`, do not work | Privilege/proc probe, host-state visibility test, parallel-task signal test, orphan reaping, full-cgroup timeout termination, and cleanup tests | Verified baseline |
| Network | Tasks have no host or external connectivity over IPv4 or IPv6, no resolver configuration, and independent runtimes never reuse a network namespace. `compile_v1` allows only socket families scoped by the network namespace (Unix, IPv4, IPv6, route netlink); `AF_VSOCK` and the rest are policy violations | Interface, route-table, IPv4/IPv6 nonblocking-connect, DNS visibility, socket-family and native socket-policy probes, and namespace-uniqueness tests | Verified baseline |
| User identity | Each task has a fresh user namespace mapping only inner 65534:65534 to one outer UID/GID leased to its request (100000–165535, distinct for every running request); supplementary groups are empty. The namespace is created by that identity, so per-user kernel limits are charged to the request and not to root or to other requests | Controller compares namespace inodes, verifies exact one-entry UID/GID maps from the probe, reads the namespace owner with `NS_GET_OWNER_UID`, and compares the identities of concurrent requests | Verified baseline |
| Privileges | Capability and identity regain, namespace-map rewriting, chroot, hostname changes, and device access fail; no setup FDs survive `exec` | Kernel-state and active privilege-escape probes | Verified baseline |
| Syscalls | Every task installs a versioned seccomp policy before `exec`; violations kill the process (`SECCOMP_RET_KILL_PROCESS`, reported as `SIGSYS`) and cannot be caught. The default `v2` profiles are allowlists: a syscall that is neither listed nor denied fails with `ENOSYS` | Probe verifies mode 2; the matrix test invokes every blocked syscall under each profile, including with a `SIGSYS` handler installed, verifies `policy_violation`, and checks that an unlisted syscall returns `ENOSYS` under `v2` | Verified baseline |
| Memory | The complete task process tree cannot exceed `memory.max`; a kill caused by the task's own limit (`out_of_memory`) is reported separately from one caused by an ancestor limit (`ancestor_out_of_memory`) | OOM acceptance test, ancestor-limit test, and reported `memory.events:oom_kill` / `memory.events.local:oom` evidence | Verified baseline |
| Request fan-out | Requests cannot exceed the configured step or parallel-task counts. `MAX_CONCURRENCY` counts task slots and a request reserves one per task of its widest step, so the service cgroup's aggregate (`(memory.max + workspace tmpfs) × slots`, `pids.max × slots`) covers every admitted task at its own limit. Each request cgroup is capped at its widest step times the per-task limits plus its workspace, so one request's pressure cannot pick a victim in another | API rejection and slot-reservation tests, request-cgroup sizing test, startup cgroup configuration | Verified baseline |
| Syscall policy | Service-configured profiles deny namespace creation, modern mount APIs, privileged handles, and x32 syscall-number bypasses | Per-syscall policy probes, clone namespace probe, clone3 fallback probe, and x32 probe | Verified denylist baseline |
| Process count | The complete task process tree cannot exceed `pids.max` | PID acceptance test and reported `pids.events:max` evidence | Verified baseline |
| CPU | CPU bandwidth, CPU time, and wall time are independently bounded | Busy-loop test verifies `cpu.max` throttling counters and a shorter `RLIMIT_CPU` terminates before wall timeout | Verified baseline |
| Rlimits | CPU time, file size, descriptors, stack, and core dumps have finite enforced policy limits; the core limit of 1 also keeps a crashing task from invoking the host's `core_pattern` pipe helper | Probe verifies configured values; active tests hit `EMFILE`, `EFBIG`, stack/core signals, absent core files, and CPU kill | Verified baseline |
| Output | stdin/stdout/stderr progress concurrently, each output stream is bounded, and the request as a whole keeps at most `REQUEST_OUTPUT_LIMIT_BYTES` of output (so every in-memory copy of a result is bounded) | Flood and bidirectional-pipe tests report `output_limit` and truncation; request-budget test shows later tasks receive the remainder and then nothing | Verified baseline |
| Request scoping | Each execution owns a `faber/req-<id>` cgroup containing all of its task cgroups; a request's overall-deadline kill is confined to that subtree and cannot terminate another request's tasks | Concurrent-request deadline test asserts the unrelated request exits normally | Verified baseline |
| Admission | A client disconnect neither frees its concurrency slot nor leaves its sandbox running: the slot is held until the runtime returns and the disconnect cancels the request cgroup | Disconnecting-client test bounds live sandboxes by the limit; abort test requires teardown well before the wall timeout | Verified baseline |
| Cleanup | No process, request or task cgroup, or container root survives success, task failure, timeout, output kill, policy violation, API cancellation, service shutdown, or partial container setup; shutdown stops later steps of running requests and bounds the drain | Outcome tests, request cancellation test, shutdown test, setup-failure root comparison, and post-run host assertions | Verified baseline |

Tasks of one request share a workspace, `/tmp`, the network and IPC
namespaces, and one outer identity, so they also share that identity's
per-user kernel limits. They do not share a PID namespace. Requests share
none of these with each other. Identities are leased per process: two Faber
services on one kernel can hand out the same one.

Each request is run by a jailer: the service executable started again as a
fresh single-threaded process that receives only the request as its input. The
jailer, the per-task supervisors and the PID namespace inits run as root in
the `supervisor` leaf of the request cgroup, so their memory is charged to the
request within a fixed allowance and they are the OOM killer's last choice
there. They hold none of the service's memory, environment or descriptors and
are killed with their parent if the service dies.

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
provides private propagation and an empty `/sys` without sysfs or the cgroup mount;
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

The test harness is single-threaded because the tests share global cgroup
fixtures; concurrent execution receives a separate stress test rather than
relying on the Rust test harness's scheduling.

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
