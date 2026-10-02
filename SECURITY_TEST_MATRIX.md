# Adversarial sandbox test matrix

This document records what the Faber namespace backend is tested against and
where its claims stop. “Pass” means the invariant was observed on the listed
kernel and CI environments; it is not a proof against unknown kernel defects.

## Tested hardware and runtimes

| Environment | Evidence | Coverage |
|---|---|---|
| Local NixOS host | x86_64, Linux 7.1.3, cgroup v2, rootful Docker 29.6.1 | Docker-only development, focused acceptance, repeated stress |
| Local CPU | AMD Ryzen 7 5825U, 8 cores/16 threads, AMD-V, `/dev/kvm` available | Native x86_64 execution; KVM is available but no Faber microVM backend exists |
| GitHub hosted VM | Ubuntu 24.04, x86_64, rootful privileged Docker | Fresh-VM full suite and five-round adversarial repetition on every push/PR |
| Production target | Debian glibc (`x86_64-unknown-linux-gnu`, plus the CI image's configured multi-arch platforms) | Compile check and production container execution |
| GitHub hosted ARM64 VM | Ubuntu 24.04, aarch64, rootful privileged Docker | The same full suite and five-round adversarial repetition on every push/PR |
| Multi-architecture images | linux/amd64 and linux/arm64 | Both published; the amd64 image is smoke-tested and run through the language toolchain test on every push/PR |

## Executable attack coverage

| Attack family | Probes and evidence |
|---|---|
| Submitted paths | Absolute paths, `..`, symlinks, proc magic links, cross-mount hard links, directories, FIFOs, Unix sockets, devices, parallel symlink swaps, and atomic `RENAME_EXCHANGE` swaps during nested directory creation |
| Root and mounts | Outer-root marker, old-root absence, private propagation, read-only toolchains, empty `/sys`, `subset=pid` procfs without host-wide files, absent cgroup filesystem, `nodev,nosuid` writable tmpfs mounts |
| Identity | User namespace owned by the request's leased identity, distinct identities for concurrent requests, UID/GID map comparison, supplementary groups, setuid/setgid/setgroups regain, map rewriting, chroot, hostname changes, capability and ambient-capability regain |
| Process visibility | Per-task PID namespace, parallel tasks unable to see or signal each other, bounded procfs process list, protected namespace PID 1, denied `/proc/1/root`, orphan/double-fork reaping |
| File descriptors | Post-`exec` enumeration permits only stdin/stdout/stderr plus the probe’s own temporary directory descriptor |
| Syscalls | Every denied syscall is invoked directly under each of the four profiles and must terminate with `SIGSYS`/`policy_violation`, also when the task handles `SIGSYS`; compile-profile socket-family and netlink-protocol rules; `v2` allowlists answer an unlisted syscall with `ENOSYS` while listed ones keep working |
| Network | Interface inventory, IPv4/IPv6 route tables, external nonblocking connects, resolver-file absence, native socket denial, unique net namespace per runtime |
| Memory and processes | Real OOM kill with `memory.events`, own-limit versus ancestor-limit OOM classification, per-request cgroup sizing, swap disabled, fork exhaustion with `pids.events`, peak values, cgroup cleanup |
| Timeout teardown | Atomic `cgroup.kill`, fork-successor stdout holders, bounded pipe grace, overall execution deadline scoped to its own request cgroup and returning completed steps with later ones marked `not_started`, no leaked request or task cgroups |
| CPU and rlimits | `cpu.max` throttling counters, independent `RLIMIT_CPU`, `EMFILE`, `EFBIG`, stack signal, zero core files |
| I/O | stdout/stderr floods, binary-size caps, truncation reporting, per-request output budget, concurrent stdin/stdout, large parallel result transport, 16 MiB result transport-time bound |
| Supervisors | Jailer, task supervisor and namespace init hold no sockets and no environment of the embedding process |
| Lifecycle | Timeout, signal, output kill, policy kill, setup failure, pre-exec failures reported by stage and errno rather than as exit codes, cancelled API request torn down immediately, shutdown stops multi-step requests and refuses new ones, disconnecting clients held to the concurrency limit, cgroup/root cleanup, concurrent distinct cgroups |

## Deliberately excluded from privileged-container tests

The following cannot be tested safely in a privileged container because it
shares the NixOS host kernel:

- public or private kernel exploits and zero-days
- deliberate kernel panic, watchdog, hung-task, or host-OOM scenarios
- speculative-execution, cache-timing, Rowhammer, and other physical side channels
- malicious firmware, DMA, device-passthrough, and hypervisor attacks
- Docker daemon, runc, or host-root compromise proofs of concept
- attacks requiring intentionally vulnerable kernel modules or filesystems

These require a disposable KVM guest image with automatic teardown and host-side
crash detection. Faber currently has no microVM backend, so passing the namespace
suite must not be represented as protection from host-kernel compromise.

## Known residual gaps

- The `v2` allowlists are derived from the Docker default profile rather than
  traced from Faber's own workloads, so a legitimate but unlisted syscall fails
  with `ENOSYS`; the `v1` denylists allow everything not known to be dangerous.
- The published ARM64 image itself is not smoke-tested (it is built under
  emulation); ARM64 is covered by running the sandbox suites natively.
- Sandbox identities are leased per service process; several Faber services on
  one kernel overlap unless each is given its own `SANDBOX_IDENTITY_BASE`.
- Namespace isolation cannot prevent kernel vulnerabilities or all denial-of-service
  and microarchitectural attacks.
