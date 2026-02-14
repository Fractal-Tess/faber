---
title: Overview
description: Understanding container isolation in Faber
---

# Container Isolation

Faber uses Linux kernel features to provide complete process isolation for task execution.

## Isolation Mechanisms

Faber employs multiple layers of isolation:

### 1. Linux Namespaces

Namespaces provide kernel-level isolation of system resources:

| Namespace | Purpose |
|-----------|---------|
| **PID** | Process ID isolation - processes in container only see their own PIDs |
| **Mount** | Filesystem isolation - separate mount points and root filesystem |
| **Network** | Network stack isolation - separate network interfaces (optional) |
| **UTS** | Hostname isolation - container has its own hostname |
| **IPC** | Inter-process communication isolation - separate IPC namespaces |

### 2. Cgroups v2

Control groups provide resource limitation and accounting:

| Controller | Function |
|------------|----------|
| **CPU** | Limit CPU usage and track CPU time |
| **Memory** | Limit RAM usage and track peak memory |
| **PIDs** | Limit process count |

### 3. Capability Dropping

All Linux capabilities are dropped after container setup:

```rust
// Clear all capabilities
capset.clear();
```

This ensures tasks run with minimal privileges.

### 4. Unprivileged User

Tasks execute as UID/GID 65534 (nobody):

```rust
// Set user to nobody
setuid(65534);
setgid(65534);
```

## Container Lifecycle

```
1. Fork process
2. Setup namespaces (clone flags)
3. Setup cgroups
4. pivot_root to isolated filesystem
5. Drop capabilities
6. Set unprivileged user
7. Execute task
8. Collect stats
9. Cleanup cgroups
10. Exit
```

## Filesystem Isolation

### pivot_root

Faber uses `pivot_root` to change the root filesystem:

1. Create minimal root filesystem
2. Mount procfs and sysfs (limited)
3. pivot_root to new root
4. Old root is unmounted

### Minimal Root Filesystem

The container filesystem includes only:

- `/bin` - Essential binaries
- `/lib` - Shared libraries
- `/lib64` - 64-bit libraries (if needed)
- `/usr` - User binaries and libraries
- `/tmp` - Temporary files
- `/dev` - Device files (minimal)
- `/proc` - Process information (limited view)

## Security Benefits

### Process Isolation

- Container processes cannot see host processes
- PID 1 in container is isolated from host PID 1
- Process trees are completely separate

### Filesystem Protection

- No access to host filesystem
- Container has its own root
- Sensitive host paths are inaccessible

### Resource Limits

- Memory limits prevent OOM on host
- CPU limits prevent resource exhaustion
- PID limits prevent fork bombs

### Privilege Reduction

- No root access within container
- All capabilities dropped
- Minimal attack surface

## Limitations

### Current Limitations

1. **No seccomp filtering** - Syscalls are not yet filtered
2. **No AppArmor/SELinux** - Mandatory access control not implemented
3. **Root required** - Container setup requires root privileges
4. **Single node** - No distributed execution

### Future Enhancements

- Syscall filtering with seccomp
- AppArmor/SELinux profiles
- Rootless execution mode
- Network policy enforcement

## Comparison with Docker

| Feature | Faber | Docker |
|---------|-------|--------|
| Startup Time | ~10ms | ~100ms |
| Memory Overhead | ~1MB | ~10MB |
| Image Size | N/A | 100MB+ |
| Isolation Level | High | High |
| Use Case | Task execution | Application deployment |

Faber is designed for fast, lightweight task execution rather than long-running applications.

## Best Practices

### For Maximum Security

1. **Always use latest version** - Security updates are released regularly
2. **Set resource limits** - Prevent resource exhaustion
3. **Validate input** - Sanitize commands and arguments
4. **Use API authentication** - Protect the API endpoint
5. **Monitor logs** - Watch for suspicious activity

### For Performance

1. **Enable caching** - Avoid duplicate executions
2. **Use parallel execution** - Run independent tasks concurrently
3. **Set appropriate limits** - Don't over-allocate resources
4. **Clean up cgroups** - Monitor `/sys/fs/cgroup/faber/`

## Next Steps

- [Resource Management](/core-concepts/resources/overview/) - Configure limits
- [Task Execution](/core-concepts/execution/overview/) - Understand execution flow
- [Docker Deployment](/deployment/docker/setup/) - Production deployment
