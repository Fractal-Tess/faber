# Cgroups Research Report

## Executive Summary

This research document provides a comprehensive overview of Linux Control Groups (cgroups), their implementation in Docker, and best practices for usage. Cgroups are a fundamental Linux kernel feature that enables resource management, isolation, and accounting for processes.

---

## Table of Contents

1. [What Are Cgroups?](#what-are-cgroups)
2. [Cgroups v1 vs v2](#cgroups-v1-vs-v2)
3. [Core Concepts](#core-concepts)
4. [Controllers](#controllers)
5. [How Cgroups Work](#how-cgroups-work)
6. [Cgroups in Docker](#cgroups-in-docker)
7. [Best Practices](#best-practices)
8. [Security Considerations](#security-considerations)
9. [References](#references)

---

## What Are Cgroups?

Control groups (cgroups) are a Linux kernel feature that allows processes to be organized into hierarchical groups whose usage of various types of resources can be limited and monitored.

### Key Capabilities

- **Resource Limiting**: Restrict how much resources (CPU, memory, I/O) a group of processes can use
- **Prioritization**: Give different groups different priorities for resource access
- **Accounting**: Measure resource usage for billing or monitoring
- **Control**: Freeze, checkpoint, and restart processes

### Why Cgroups Matter

From the Linux kernel documentation:

> "Control groups, usually referred to as cgroups, are a Linux kernel feature which allow processes to be organized into hierarchical groups whose usage of various types of resources can then be limited and monitored."

Cgroups are essential for:
- Container isolation (Docker, Kubernetes)
- Multi-tenant environments
- Preventing "noisy neighbor" problems
- Resource accounting and billing
- System stability and DoS protection

---

## Cgroups v1 vs v2

### Cgroups v1 (Legacy)

- Multiple separate hierarchies for different controllers
- Each controller could have its own mount point
- Complex to manage and understand
- Still present on older systems

### Cgroups v2 (Current)

- Unified single hierarchy for all controllers
- Simplified management model
- Better resource isolation
- Required for rootless containers
- Default in modern distributions (Ubuntu 22.04+, recent RHEL)

### Checking Your Version

```bash
# Check which cgroup version is mounted
stat -fc %T /sys/fs/cgroup/

# Expected output for v2:
# cgroup2fs

# For v1, you would see:
# tmpfs
```

### Migration Timeline

- **v2 introduced**: Linux kernel 4.5 (2016)
- **Became default**: Recent distributions (2020+)
- **v1 deprecated**: Many distributions are phasing out v1 support

---

## Core Concepts

### The cgroup Filesystem

The cgroup interface is exposed through a pseudo-filesystem mounted at `/sys/fs/cgroup`:

```bash
mount -l | grep cgroup
# cgroup2 on /sys/fs/cgroup type cgroup2 ...
```

### Hierarchy

Cgroups are organized hierarchically:

```
/sys/fs/cgroup/
├── init.scope          # System init processes
├── system.slice        # System services
│   ├── nginx.service
│   └── ssh.service
├── user.slice          # User sessions
│   └── user-1000.slice
│       └── session-1.scope
└── docker/             # Docker containers
    └── <container-id>
```

### Key Files

Each cgroup directory contains special files:

**Core Interface Files** (starting with `cgroup.`):
- `cgroup.procs` - List of processes in the cgroup
- `cgroup.threads` - List of threads in the cgroup
- `cgroup.controllers` - Available controllers
- `cgroup.subtree_control` - Enable controllers for children
- `cgroup.events` - Event notifications
- `cgroup.kill` - Kill all processes (write 1)
- `cgroup.type` - Type of cgroup (domain, threaded, etc.)

**Controller Files** (controller-specific):
- `cpu.max` - CPU time limits
- `memory.max` - Memory hard limit
- `memory.high` - Memory throttling threshold
- `pids.max` - Maximum number of processes
- `io.max` - I/O bandwidth limits

### Process Assignment

Processes are assigned to cgroups by writing their PID:

```bash
# Add process to cgroup
echo <PID> >> /sys/fs/cgroup/mygroup/cgroup.procs

# Child processes inherit parent's cgroup
# Moving parent moves children (unless explicitly placed elsewhere)
```

---

## Controllers

Controllers (also called subsystems) enforce resource limits. Common controllers include:

### CPU Controller

Controls CPU time allocation:

```bash
# cpu.max format: "quota period"
# quota: maximum CPU time per period (in microseconds)
# period: length of period (in microseconds)
# "max" means no limit

# Limit to 50% of one CPU
echo "50000 100000" > /sys/fs/cgroup/mygroup/cpu.max

# Limit to 2 full CPUs
echo "200000 100000" > /sys/fs/cgroup/mygroup/cpu.max

# No limit
echo "max 100000" > /sys/fs/cgroup/mygroup/cpu.max
```

**CPU Weight** (for relative prioritization):
```bash
# cpu.weight range: 1-10000, default 100
echo "200" > /sys/fs/cgroup/mygroup/cpu.weight
```

### Memory Controller

Controls memory usage:

```bash
# Hard limit - OOM kill when exceeded
echo "100M" > /sys/fs/cgroup/mygroup/memory.max

# Soft limit - throttle when exceeded (but not kill)
echo "80M" > /sys/fs/cgroup/mygroup/memory.high

# Minimum guaranteed memory
echo "50M" > /sys/fs/cgroup/mygroup/memory.min

# Swap limit
echo "200M" > /sys/fs/cgroup/mygroup/memory.swap.max
```

**Memory Events**:
- `memory.events` - High-level events (oom, oom_kill, etc.)
- `memory.events.local` - Events for this cgroup only
- `memory.current` - Current memory usage
- `memory.stat` - Detailed statistics

### PIDs Controller

Limits number of processes:

```bash
# Maximum processes in cgroup
echo "100" > /sys/fs/cgroup/mygroup/pids.max

# Current count
cat /sys/fs/cgroup/mygroup/pids.current
```

Prevents fork bombs and runaway process creation.

### IO Controller

Controls block I/O:

```bash
# Limit I/O bandwidth for specific devices
# Format: "device_major:device_minor rbps=... wbps=..."
echo "8:16 rbps=10485760 wbps=10485760" > /sys/fs/cgroup/mygroup/io.max

# IO weight (relative priority)
echo "200" > /sys/fs/cgroup/mygroup/io.weight
```

### Other Controllers

- **cpuset**: Pin processes to specific CPUs and memory nodes
- **hugetlb**: Limit huge page usage
- **rdma**: Limit RDMA/InfiniBand resources
- **misc**: Miscellaneous resources

---

## How Cgroups Work

### Creating a cgroup

```bash
# Create a new cgroup by making a directory
mkdir /sys/fs/cgroup/my_cgroup

# Directory is automatically populated with controller files
ls /sys/fs/cgroup/my_cgroup
```

### Configuring Resources

```bash
# Set CPU limit to 50%
echo "50000 100000" > /sys/fs/cgroup/my_cgroup/cpu.max

# Set memory limit to 100MB
echo "100M" > /sys/fs/cgroup/my_cgroup/memory.max

# Set max processes to 50
echo "50" > /sys/fs/cgroup/my_cgroup/pids.max
```

### Running Processes

```bash
# Method 1: Move existing process
echo $$ >> /sys/fs/cgroup/my_cgroup/cgroup.procs
./my_program

# Method 2: Start process in cgroup (child inherits)
cgexec -g cpu,memory:my_cgroup ./my_program

# Method 3: Using systemd
systemd-run --scope --property=CPUQuota=50% --property=MemoryMax=100M ./my_program
```

### Cleanup

```bash
# Remove cgroup (must be empty)
rmdir /sys/fs/cgroup/my_cgroup

# Or using cgdelete
cgdelete -g cpu,memory:/my_cgroup
```

### Monitoring

```bash
# View cgroup hierarchy
systemd-cgls

# Monitor resource usage
systemd-cgtop

# Get cgroup configuration
cgget my_cgroup

# View process cgroup
cat /proc/<PID>/cgroup
```

---

## Cgroups in Docker

### How Docker Uses Cgroups

Docker leverages cgroups to:
1. **Isolate resources** per container
2. **Enforce limits** on CPU, memory, I/O
3. **Account for usage** (monitoring/billing)
4. **Prevent DoS** attacks via resource exhaustion

### Docker Resource Constraints

Docker provides flags to control cgroup settings:

#### Memory Constraints

```bash
# Hard memory limit
docker run -m 512m nginx

# Memory + swap limit
docker run -m 512m --memory-swap 1g nginx

# Memory reservation (soft limit)
docker run -m 512m --memory-reservation 256m nginx

# Disable OOM killer
docker run -m 512m --oom-kill-disable nginx

# Swappiness (0-100)
docker run --memory-swappiness 50 nginx
```

#### CPU Constraints

```bash
# Limit to 1.5 CPUs
docker run --cpus="1.5" nginx

# CPU shares (relative weight, default 1024)
docker run --cpu-shares 512 nginx

# CPU period and quota
docker run --cpu-period=100000 --cpu-quota=50000 nginx

# Pin to specific CPUs
docker run --cpuset-cpus="0,2" nginx
docker run --cpuset-cpus="0-3" nginx
```

#### PID Limits

```bash
# Limit to 100 processes
docker run --pids-limit 100 nginx
```

#### Block I/O

```bash
# Block IO weight
docker run --blkio-weight 300 nginx

# Device-specific limits
docker run --device-read-bps /dev/sda:1mb nginx
docker run --device-write-iops /dev/sda:1000 nginx
```

### Docker's Cgroup Hierarchy

Docker creates cgroups under `/sys/fs/cgroup/docker/`:

```
/sys/fs/cgroup/docker/
├── <container-id>/
│   ├── cpu.max
│   ├── memory.max
│   ├── pids.max
│   └── ...
└── ...
```

### Viewing Container Cgroups

```bash
# Get container PID
PID=$(docker inspect -f '{{.State.Pid}}' <container>)

# View cgroup path
cat /proc/$PID/cgroup

# View cgroup files
ls /sys/fs/cgroup/docker/<container-id>/

# Check CPU limit
cat /sys/fs/cgroup/docker/<container-id>/cpu.max

# Check memory limit
cat /sys/fs/cgroup/docker/<container-id>/memory.max
```

### Docker with Systemd

Modern Docker can use systemd as the cgroup driver:

```bash
# Check current driver
docker info | grep "Cgroup Driver"

# Configure in daemon.json
{
  "exec-opts": ["native.cgroupdriver=systemd"]
}
```

Benefits:
- Single cgroup manager (systemd)
- Better integration with systemd services
- Required for rootless containers

---

## Best Practices

### General Guidelines

1. **Always set memory limits**
   - Prevents OOM situations that affect the host
   - Set both `--memory` and `--memory-swap`

2. **Use CPU requests (shares) for prioritization**
   - Shares only matter under contention
   - Use `--cpus` for hard limits

3. **Set PID limits**
   - Prevents fork bombs
   - Protects against runaway processes

4. **Monitor resource usage**
   - Use `docker stats`
   - Set up alerts for resource exhaustion

### Memory Best Practices

```bash
# Good: Set memory limit with swap
docker run -m 512m --memory-swap 512m app

# Better: Also set reservation for guaranteed memory
docker run -m 512m --memory-reservation 256m --memory-swap 512m app
```

**Memory Swap Considerations**:
- `--memory-swap` = total memory + swap
- Set equal to `--memory` to disable swap
- Swap is slower but prevents OOM kills

### CPU Best Practices

```bash
# For guaranteed CPU
docker run --cpus="2.0" app

# For relative priority (when resources are constrained)
docker run --cpu-shares 2048 app  # 2x default

# For CPU pinning (performance)
docker run --cpuset-cpus="0-3" app
```

### Production Recommendations

1. **Set resource limits on all containers**
   ```yaml
   # docker-compose example
   services:
     app:
       deploy:
         resources:
           limits:
             cpus: '1.5'
             memory: 512M
           reservations:
             cpus: '0.5'
             memory: 256M
   ```

2. **Use reservations for critical services**
   - Ensures minimum resources available
   - Prevents starvation

3. **Monitor and tune**
   - Start with generous limits
   - Monitor actual usage
   - Tighten based on real data

4. **Consider the trade-offs**
   - Limits prevent resource exhaustion
   - But may cause throttling or OOM kills
   - Test under realistic load

### Kubernetes Integration

```yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: app
    resources:
      requests:
        memory: "256Mi"
        cpu: "250m"
      limits:
        memory: "512Mi"
        cpu: "500m"
```

**Key Points**:
- `requests` = minimum guaranteed (used for scheduling)
- `limits` = maximum allowed (enforced by cgroups)
- CPU limits use cgroups quota/period
- Memory limits use cgroups memory.max

---

## Security Considerations

### DoS Protection

Cgroups help prevent denial-of-service attacks:

1. **Fork Bomb Protection**
   ```bash
   docker run --pids-limit 100 vulnerable-app
   ```

2. **Memory Exhaustion**
   ```bash
   docker run -m 512m --memory-swap 512m app
   ```

3. **CPU Exhaustion**
   ```bash
   docker run --cpus="1.0" app
   ```

### Container Escape Prevention

- Cgroups alone don't prevent container escapes
- Combine with other security features:
  - Namespaces (PID, network, mount)
  - Capabilities dropping
  - Seccomp profiles
  - AppArmor/SELinux

### Privilege Considerations

- Modifying cgroups requires privileges
- Rootless containers use user namespaces + cgroups v2
- Delegation allows unprivileged cgroup management

### Device Access Control

```bash
# Grant specific device access
docker run --device /dev/dm-0 app

# In cgroups v2, eBPF programs manage device access
# Use bpftool to inspect:
bpftool cgroup tree /sys/fs/cgroup/docker/<container-id>
```

---

## References

### Official Documentation

- [Linux Kernel cgroup v2 Documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [cgroups(7) - Linux Man Pages](https://man7.org/linux/man-pages/man7/cgroups.7.html)
- [Docker Resource Constraints](https://docs.docker.com/engine/containers/resource_constraints/)

### Tutorials and Guides

- [Controlling Process Resources with Linux Control Groups (iximiuz)](https://labs.iximiuz.com/tutorials/controlling-process-resources-with-cgroups)
- [A Journey to Understand cgroups v2 (Fernando Villalba)](https://fernandovillalba.substack.com/p/a-journey-to-understand-cgroups-v2)
- [Container Security Fundamentals: Cgroups (Datadog)](https://securitylabs.datadoghq.com/articles/container-security-fundamentals-part-4/)

### Tools

- `cgcreate`, `cgset`, `cgexec`, `cgdelete` - libcgroup tools
- `systemd-cgls` - List cgroup hierarchy
- `systemd-cgtop` - Monitor cgroup resource usage
- `lscgroup` - List cgroups

### Further Reading

- [systemd Cgroup Delegation](https://systemd.io/CGROUP_DELEGATION/)
- [Facebook cgroup2 Documentation](https://facebookmicrosites.github.io/cgroup2/docs/overview.html)
- [Kubernetes Cgroups Documentation](https://kubernetes.io/docs/concepts/architecture/cgroups/)

---

## Summary

Cgroups are a fundamental Linux kernel feature for resource management. Key takeaways:

1. **Cgroups v2** is the modern standard with unified hierarchy
2. **Controllers** enforce limits on CPU, memory, I/O, and PIDs
3. **Docker** uses cgroups for container resource isolation
4. **Always set limits** in production to prevent resource exhaustion
5. **Monitor usage** and tune limits based on real data
6. **Combine with other security features** for defense in depth

Understanding cgroups is essential for anyone working with containers, Kubernetes, or Linux resource management.
