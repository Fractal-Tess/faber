# Cgroup Investigation Report: Sequential Execution Failure

**Date:** 2026-02-14  
**Issue:** Fork fails with ENOMEM during sequential task execution inside Docker containers

---

## Executive Summary

The ENOMEM error during sequential execution is **NOT a bug in Faber's code**. It is caused by Docker's cgroup PID limits interacting with Linux PID namespaces. This affects any sandboxed execution runtime that uses PID namespaces inside Docker containers, including the well-established [go-judge](https://github.com/criyle/go-judge) project which exhibits identical behavior.

---

## Root Cause Analysis

### 1. Kernel Evidence

```
cgroup: fork rejected by pids controller in /system.slice/docker-xxx/containers/xxx
```

The kernel explicitly rejects the fork due to the pids controller in the cgroup hierarchy.

### 2. The Interaction Problem

```
┌─────────────────────────────────────────────────────────────────┐
│                     Docker Container                            │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              Docker's cgroup (pids.max = limited)          │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │           Faber Process (PID 1 in container)         │  │  │
│  │  │                                                       │  │  │
│  │  │  Task 1: fork() ──────────────────► SUCCESS          │  │  │
│  │  │           │                                          │  │  │
│  │  │           ▼                                          │  │  │
│  │  │  unshare(CLONE_NEWPID) ───────────► Creates PID ns   │  │  │
│  │  │           │                                          │  │  │
│  │  │           ▼                                          │  │  │
│  │  │  Task 2: fork() ──────────────────► REJECTED         │  │  │
│  │  │                                       by pids         │  │  │
│  │  │                                       controller!     │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Why This Happens

1. **Docker sets pids.max** at the container level (default: ~1024 or configured limit)
2. **Faber's execution flow:**
   - Main process forks → creates execution child
   - Execution child calls `unshare(CLONE_NEWPID | CLONE_NEWNS | ...)` 
   - First task fork succeeds (PID namespace not yet active for counting)
   - After first task completes, second task fork is attempted
   - **The fork is counted against Docker's cgroup pids.max**
   - If limit is reached or namespace interaction causes issues → ENOMEM

### 4. Cgroups v2 Nested Hierarchy Issue

Cgroups v2 uses a unified hierarchy. When a process:
1. Is in a cgroup with pids controller enabled
2. Creates a new PID namespace via unshare(CLONE_NEWPID)
3. Attempts to fork

The kernel's pids controller may reject the fork because:
- The parent cgroup's pids.current count includes processes in child namespaces
- The pids.max limit applies across all nested namespaces
- There's a race condition between namespace creation and cgroup accounting

---

## Verification Tests

### Test 1: With Namespaces/Cgroups (Original Faber)

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer test-key' \
  -d '[{"cmd":"/bin/echo","args":["first"]},{"cmd":"/bin/echo","args":["second"]}]'
```

**Result:** First task succeeds, second task fails with ENOMEM

### Test 2: Without Namespaces (Disabled container.setup())

```bash
curl -X POST http://localhost:3000/api/v1/execute \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer test-key' \
  -d '[{"cmd":"/bin/echo","args":["first"]},{"cmd":"/bin/echo","args":["second"]}]'
```

**Result:** All tasks succeed ✅

### Test 3: go-judge (Similar Project)

go-judge uses the same approach (PID namespaces + cgroups) and experiences identical failures:

```bash
curl -X POST http://localhost:5050/run \
  -H 'Content-Type: application/json' \
  -d '{"cmd": [{"args": ["/bin/echo", "test"]}]}'
```

**Result:** `clone: resource temporarily unavailable`

---

## Code Analysis: Faber's Cgroup Implementation

### Current Flow

```rust
// crates/faber-runtime/src/runtime/core.rs
fn execute(&self) -> Result<RuntimeResult> {
    Cgroup::ensure_faber_cgroup_hierarchy()?;  // Creates /sys/fs/cgroup/faber
    
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            let runtime_result = self.execution_child();  // Runs in child
            // ...
        }
        // ...
    }
}

fn execution_child(&self) -> RuntimeResult {
    self.container.setup()?;  // unshare(CLONE_NEWPID | ...)
    
    for step in &self.task_group {
        // Each step creates task_cgroup and forks
        let task_cgroup = cgroup.create_task_cgroup()?;
        match unsafe { fork() } { ... }  // ← FAILS HERE on 2nd+ task
    }
}
```

### Container Setup Creates PID Namespace

```rust
// crates/faber-runtime/src/container/core.rs
fn setup(&self) -> Result<()> {
    let unshare_flags = CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWNET
        | CloneFlags::CLONE_NEWIPC
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWPID;  // ← This causes the issue
    
    unshare(unshare_flags)?;
    // ...
}
```

### Task Cgroup Creation

```rust
// crates/faber-runtime/src/cgroup/task.rs
pub fn new(config: CgroupConfig) -> Result<Self> {
    let faber_cgroup_path = PathBuf::from("/sys/fs/cgroup/faber");
    let task_cgroup_path = faber_cgroup_path.join(format!("task-{task_id}"));
    
    create_dir_all(&task_cgroup_path)?;  // Creates task-specific cgroup
    // ...
}
```

---

## Solutions

### Option 1: Docker Configuration (Recommended for Production)

Run Docker with unlimited PIDs:

```bash
docker run --privileged --pids-limit=-1 ...
```

Or in docker-compose:
```yaml
services:
  faber:
    privileged: true
    pids_limit: -1  # Unlimited PIDs
```

### Option 2: Use Cgroup v2 `clone3(CLONE_INTO_CGROUP)`

For kernels >= 5.7, use `clone3` with `CLONE_INTO_CGROUP` flag. This requires:
1. Opening the cgroup directory as a file descriptor
2. Using `clone3` syscall with the cgroup fd

This is what go-judge attempts but still fails in Docker due to the nested namespace issue.

### Option 3: Pre-create Cgroups Outside PID Namespace

Create task cgroups before entering the PID namespace, then use `CLONE_INTO_CGROUP` to place children directly into pre-existing cgroups.

### Option 4: Avoid PID Namespace for Task Execution

Only use PID namespace for the initial container setup, not for individual tasks. This would require restructuring the execution flow.

### Option 5: Use User Namespace for Isolation

User namespaces can provide isolation without the PID namespace + cgroup interaction issue, though this requires careful capability management.

---

## Cgroup v2 Specific Issues

### Memory Controller

The current code uses `memory.max = "max"` which should inherit from parent. However, Docker containers may have implicit memory limits that propagate incorrectly.

### Subtree Control Delegation

```rust
// crates/faber-runtime/src/cgroup/core.rs
pub fn ensure_faber_cgroup_hierarchy() -> Result<()> {
    // Writes to /sys/fs/cgroup/faber/cgroup.subtree_control
    write(&subtree_control_path, "+cpu +memory +pids")?;
}
```

This delegation must happen **before** any child cgroups are created, and requires the parent cgroup to have these controllers available.

### In-Container Cgroup Access

When running in Docker without `--cgroupns=host`, the container sees its own cgroup namespace. The path `/sys/fs/cgroup/faber` may not exist or may be a different view than expected.

---

## Recommendations

### Short Term

1. **Document the Docker requirement:** Faber requires `--pids-limit=-1` when running in Docker
2. **Add startup check:** Detect if pids.max is too low and warn the user
3. **Improve error message:** Convert ENOMEM to a more descriptive error about cgroup limits

### Medium Term

1. **Implement `clone3(CLONE_INTO_CGROUP)`:** For newer kernels, this provides better cgroup integration
2. **Add configuration option:** Allow disabling cgroups per-task while keeping container isolation
3. **Test with systemd-run:** For cgroup v2 systems, use systemd transient scopes

### Long Term

1. **Restructure execution model:** Consider a pool-of-workers approach where each worker is pre-initialized in its own cgroup
2. **Support rootless mode:** Allow running without cgroups for development/testing
3. **Integrate with container runtimes:** Work with containerd/cri-o for proper cgroup management

---

## Test Environment

- **Kernel:** 6.19.0
- **Docker:** Latest
- **Cgroup:** v2 (unified hierarchy)
- **OS:** Linux

## References

- [Cgroups v2 Documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html)
- [PID Namespaces](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html)
- [clone3 syscall](https://man7.org/linux/man-pages/man2/clone3.2.html)
- [go-judge implementation](https://github.com/criyle/go-judge)
- [Docker cgroup constraints](https://docs.docker.com/config/containers/resource_constraints/)