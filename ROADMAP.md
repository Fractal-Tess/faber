# Faber completion roadmap

Faber has a workable namespace and cgroup foundation, but it is not yet safe to
expose as a public untrusted-code service. The near-term goal is a small,
testable jailer; the longer-term goal is an execution service with explicit
security tiers and content-addressed artifacts.

## Threat model

An attacker controls command paths, arguments, environment variables, stdin,
uploaded files, source code, and binaries. They may fork indefinitely, exhaust
memory or output, exploit parser and compiler bugs, probe the kernel, race file
operations, and attempt to observe another tenant. The host kernel, outer
runtime image, Faber daemon, API key configuration, and pinned toolchains are
trusted.

The namespace backend must contain ordinary hostile programs and bound their
resource use. It cannot protect against every host-kernel vulnerability or
microarchitectural side channel. Workloads requiring stronger isolation should
use a gVisor or microVM backend rather than receiving weaker guarantees from the
same endpoint.

## Current baseline

Implemented:

- PID, mount, network, UTS, and IPC namespaces
- `pivot_root`, read-only toolchain bind mounts, tmpfs work and temporary dirs
- UID/GID drop and effective/permitted/inheritable capability clearing
- cgroup v2 CPU bandwidth, memory, PID limits, metrics, and `Drop` cleanup
- wall-clock task timeout, sequential and parallel task groups
- content-addressed memory, filesystem, and hybrid file stores
- Docker-only hot reload, testing, health checks, and debugger attachment

Not production-ready:

- seccomp is a no-op and no user namespace exists
- supplementary groups, rlimits, output limits, and robust PID 1 reaping are
  missing
- task file paths are not constrained beneath the workspace
- `/sys` has a host bind fallback and several mount errors are discarded
- the in-memory execution cache has no TTL, bound, persistence, single-flight,
  tenant scope, or cacheability contract
- resource enforcement, timeout, cleanup, and breakout behavior lack adversarial
  tests in CI

Whole-execution caching is disabled by default until an explicit cacheability
contract exists. Replaying `date`, random output, external state, or stale
resource statistics is not layer caching.

## Execution slices

Each slice must add or update executable acceptance tests, pass the Docker-only
validation workflow, and be committed and pushed before work starts on the next
slice.

- [x] Verification foundation: Docker workflow, threat model, invariant matrix,
  safe workspace files, and real memory/PID enforcement tests
- [x] Security-state probe: identity maps, groups, capabilities, `NoNewPrivs`,
  seccomp, namespaces, mounts, routes, rlimits, and cgroup membership
- [x] Privilege cleanup: supplementary groups, ambient/bounding capabilities,
  and `NoNewPrivs`
- [x] Bounded process I/O: concurrent draining, stream limits, truncation, and
  output-limit tests
- [x] Process lifecycle: PID 1 reaping, process-tree termination, timeout, and
  zombie tests (API cancellation remains part of explicit execution outcomes)
- [x] Mount/sys hardening: private propagation, read-only sysfs, removal of
  host fallbacks, and fail-closed tests
- [x] Rlimits: CPU/file/FD/core/stack limits with probe assertions
- [ ] Resource outcomes: explicit OOM, PID, timeout, signal, output, and cleanup
  outcomes backed by cgroup event files
- [ ] User namespace: explicit UID/GID mappings with host-side evidence
- [ ] Versioned seccomp profiles: compile/native policies and violation tests
- [ ] Adversarial CI: ordinary quality gates plus disposable-VM isolation,
  lifecycle, and concurrency suites

## Phase 0: reproducible development

Status: local Docker workflow complete; CI expansion remains planned.

- Use rootful Docker; reject rootless Docker because nested mount operations do
  not receive the required capabilities.
- Run and test only through `scripts/dev.sh`.
- Keep Cargo target, registry, and git data in named volumes.
- Run the same privileged/cgroup configuration in development and integration
  tests.
- Planned: add Rust format, Clippy, unit, container integration, SDK, and docs
  checks to pull-request CI.

## Phase 1: close immediate containment gaps

Complete these before accepting hostile workloads:

1. **Baseline complete:** submitted files are constrained to normalized,
   workspace-relative regular files using `openat2` with `RESOLVE_BENEATH`,
   `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`, and `RESOLVE_NO_XDEV`.
   Acceptance tests cover absolute paths, `..`, and adversarial workspace
   symlinks; disposable-VM race stress remains planned.
2. **Complete:** poll stdin/stdout/stderr concurrently while the child runs,
   cap each output stream, report truncation, and kill the full task cgroup when
   either budget is exceeded. Flood and bidirectional pipe tests cover the
   former wait-before-read deadlock.
3. **Complete:** apply fail-closed rlimits for CPU time, file size, open file
   descriptors, stack, and core dumps. Cgroups remain the authoritative
   aggregate process-tree limits.
4. **Complete:** clear supplementary groups and ambient/bounding capabilities,
   then set `NoNewPrivs` before execution. The security-state probe verifies
   every resulting kernel field.
5. **Complete for execution and timeout:** PID 1 continuously reaps orphaned
   descendants and timeout/output enforcement kills the full task cgroup.
   API-level cancellation remains pending with explicit outcomes.
6. **Complete:** mount propagation is private, task sysfs is read-only, fresh
   sysfs/cgroup mounts have no old-root bind fallback, and setup retains mount
   errors instead of silently continuing.
7. Add explicit execution outcomes: exited, signaled, timed out, OOM-killed,
   PID-limited, output-limited, setup-failed, and policy-violated.

Acceptance tests must include path traversal and symlink races, output floods,
fork bombs, OOM, CPU and wall time limits, leaked descendants, read-only mounts,
capability state, host process visibility, and cleanup after every failure path.

## Phase 2: syscall policy and privilege model

Use the existing `seccompiler` dependency rather than maintaining a BPF
compiler. `seccompiler::apply_filter` sets `PR_SET_NO_NEW_PRIVS` while installing
the filter.

Create versioned execution profiles instead of one universal allowlist:

- `compile`: process creation and filesystem operations needed by pinned
  compilers, with no network access
- `native`: minimal dynamic-loader, memory, signal, time, and stdio surface
- language-specific runtime profiles for JVM, Node, Python, and other supported
  toolchains

Unknown syscalls should initially return a policy error in development so
coverage can be measured, then use a terminating action in hardened profiles.
Trace representative workloads, but treat traces as test input rather than proof
that a policy is complete. Include architecture and policy version in every
execution specification and cache key.

Add a user namespace with explicit UID/GID maps after the existing mount flow is
covered by tests. Do not combine this migration with the first seccomp patch;
each boundary should have an independently reviewable regression suite.

## Phase 3: resource and lifecycle correctness

- Distinguish CPU bandwidth (`cpu.max`) from total CPU time (rlimit and sampled
  `cpu.stat`).
- Make memory and PID limits mandatory per service policy rather than defaulting
  memory to unlimited.
- Read `memory.events`, `pids.events`, PSI, and signal status before cgroup
  cleanup so results explain why a task ended.
- Use cancellation-safe RAII for containers, mounts, cgroups, pipes, and child
  processes; report cleanup failures to metrics.
- Add queue limits and backpressure in addition to HTTP concurrency limits.

## Phase 4: artifact and compile caching

Keep three separate concepts:

1. **CAS blobs**: immutable bytes addressed by SHA-256 descriptors containing
   digest, size, media type, and executable bit. Verify size and digest on read.
2. **Action cache**: a canonical action digest mapped to a manifest of output
   blob descriptors. The action includes command, ordered arguments, sorted
   environment, Merkle input tree, toolchain image digest, architecture,
   sandbox policy version, mounts, limits, and timeout.
3. **Ephemeral snapshot**: a fresh overlayfs upper/work directory over immutable
   base and toolchain lower layers. Never share writable layers between jobs.

The first useful feature is compile-once/run-many artifact caching, not cached
stdout. Extend tasks with content-addressed inputs, declared output paths, and a
cache policy. On a compile hit, materialize verified output blobs into a fresh
workspace; always execute the binary in a fresh sandbox.

Use the existing filesystem store as the initial CAS after adding immutable
descriptors, read verification, quotas, TTL/GC, and leases for in-flight work.
Add single-flight by action digest so concurrent identical compilations share
one producer. Scope metadata and result caches by tenant; cross-tenant artifact
deduplication must be an explicit confidentiality policy, not an accident.

Do not cache arbitrary execution results by default. Permit result caching only
for actions marked deterministic and include all inputs and runtime policy in
the action digest. Do not cache failed, timed-out, policy-violating, or
output-truncated executions unless a caller explicitly requests negative-cache
semantics.

## Phase 5: production isolation tiers

Retain the namespace backend for low-latency, lower-risk workloads after Phases
1-3 pass. Add a stronger backend behind the same action contract:

- gVisor for OCI-compatible workloads needing a reduced host syscall surface
- Firecracker or another microVM runtime for hostile native binaries and
  high-assurance tenants

Faber should own admission, action manifests, CAS, scheduling, results, and
observability. It should not reimplement an OCI image builder, BPF compiler, or
virtual machine monitor.

## Release gate

A public release requires:

- a written threat model and versioned sandbox profiles
- zero known fail-open paths
- bounded CPU, memory, PIDs, files, descriptors, wall time, and output
- adversarial integration tests running in rootful Docker on every pull request
- cache quotas, integrity verification, tenant partitioning, and garbage
  collection
- an operator runbook for leaked cgroups/mounts, OOM storms, policy violations,
  and incident response
- an explicit statement of the isolation tier and residual kernel risk in every
  deployment guide
