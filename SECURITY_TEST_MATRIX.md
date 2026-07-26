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
| Production target | `x86_64-unknown-linux-musl` | Compile check and production container execution |
| Multi-architecture images | linux/amd64 and linux/arm64 | Build-only for ARM64; ARM64 sandbox behavior is not runtime-tested |

## Executable attack coverage

| Attack family | Probes and evidence |
|---|---|
| Submitted paths | Absolute paths, `..`, symlinks, proc magic links, cross-mount hard links, directories, FIFOs, Unix sockets, devices, and parallel symlink swaps |
| Root and mounts | Outer-root marker, old-root absence, private propagation, read-only toolchains/sysfs, absent cgroup filesystem, `nodev,nosuid` writable tmpfs mounts |
| Identity | UID/GID map comparison, supplementary groups, setuid/setgid/setgroups regain, map rewriting, chroot, hostname changes, capability and ambient-capability regain |
| Process visibility | PID namespace inode, bounded procfs process list, protected namespace PID 1, denied `/proc/1/root`, orphan/double-fork reaping |
| File descriptors | Post-`exec` enumeration permits only stdin/stdout/stderr plus the probe’s own temporary directory descriptor |
| Syscalls | Every syscall entry in `compile_v1` and `native_v1` is invoked directly and must terminate with `SIGSYS`/`policy_violation` |
| Network | Interface inventory, IPv4/IPv6 route tables, external nonblocking connects, resolver-file absence, native socket denial, unique net namespace per runtime |
| Memory and processes | Real OOM kill with `memory.events`, swap disabled, fork exhaustion with `pids.events`, peak values, cgroup cleanup |
| CPU and rlimits | `cpu.max` throttling counters, independent `RLIMIT_CPU`, `EMFILE`, `EFBIG`, stack signal, zero core files |
| I/O | stdout/stderr floods, binary-size caps, truncation reporting, concurrent stdin/stdout, large parallel result transport |
| Lifecycle | Timeout, signal, output kill, policy kill, setup failure, detached API request, cgroup/root cleanup, concurrent distinct cgroups |

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

- Seccomp profiles are versioned denylists, not exhaustive allowlists.
- ARM64 is build-tested but not runtime-tested.
- API cancellation detaches the blocking runtime; cleanup is verified after the
  configured wall timeout rather than immediate cooperative cancellation.
- The API still forks from a multithreaded service process and performs setup
  before `exec`; a dedicated single-threaded jailer remains the safer design.
- Namespace isolation cannot prevent kernel vulnerabilities or all denial-of-service
  and microarchitectural attacks.
