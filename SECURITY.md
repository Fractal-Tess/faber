# Faber security model and verification

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
| Workspace files | Submitted paths are normalized, relative to `/faber`, and cannot traverse symlinks or mount points | `security_acceptance::submitted_files_*`; `openat2` with `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`, and `RESOLVE_NO_XDEV` | Verified baseline |
| Root filesystem | The old root is detached, outer-root files are hidden, and toolchain mounts cannot be modified | `filesystem_hides_outer_root_and_keeps_toolchains_read_only`; `test_toolchain_mounts_are_read_only` | Verified baseline |
| PID/proc | Host processes are absent from procfs and all descendants are reaped and killed at teardown | `test_pid_namespace_isolation` and the security-state probe cover visibility; descendant/reaper tests pending | Partial |
| Network | Tasks have no host or external connectivity over IPv4 or IPv6 | Interface smoke test exists; route, DNS, socket, and cross-task tests pending | Partial |
| User identity | Task IDs map to an unprivileged host UID/GID and supplementary groups are empty | Security-state probe verifies 65534:65534 and an empty supplementary-group list; user namespace pending | Partial |
| Privileges | Effective, permitted, inheritable, ambient, and bounding capabilities are empty; `NoNewPrivs` is set | Security-state probe verifies all five capability sets and `NoNewPrivs: 1` | Verified baseline |
| Syscalls | A versioned workload policy denies syscalls outside the declared profile | `apply_seccomp_filter()` remains a no-op | Not implemented |
| Memory | The complete task process tree cannot exceed `memory.max` | `memory_cgroup_kills_a_process_that_exceeds_memory_max` | Verified baseline |
| Process count | The complete task process tree cannot exceed `pids.max` | `pids_cgroup_enforces_the_process_limit` | Verified baseline |
| CPU | CPU bandwidth and total CPU/wall time are independently bounded | Throttling counter and CPU-time tests pending | Partial |
| Output | stdin/stdout/stderr progress concurrently and each output stream is bounded | Flood and bidirectional-pipe acceptance tests verify cgroup termination and truncation reporting | Verified baseline |
| Cleanup | No process, cgroup, mount, or workspace survives any completion path | Failure-mode and host-observer tests pending | Partial |

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
supplementary and capability sets with `NoNewPrivs: 1`; later slices address the
remaining observations.

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

Destructive abuse tests, mount propagation probes, aggressive fork/OOM stress,
and future escape-oriented tests belong on disposable CI VMs. The VM must be
destroyed after the suite. Do not run kernel exploits as sandbox tests.

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
