---
title: Resource Management
description: Understanding resource limits and monitoring
---

# Resource Management

Faber uses cgroups v2 to manage and monitor resource usage.

## Cgroup Controllers

Faber supports three cgroup controllers:

### CPU Controller

Limits CPU usage and tracks CPU time:

- **cpu.max** - Maximum CPU bandwidth
- **cpu.stat** - CPU usage statistics

### Memory Controller

Limits memory usage and tracks peak memory:

- **memory.max** - Maximum memory limit
- **memory.peak** - Peak memory usage

### PIDs Controller

Limits process count:

- **pids.max** - Maximum number of processes
- **pids.peak** - Peak process count

## Default Limits

Without explicit configuration, Faber uses system defaults.

## Resource Statistics

Every task result includes resource statistics:

```json
{
  "stats": {
    "memory_peak_bytes": 1048576,
    "cpu_usage_usec": 12345,
    "pids_peak": 1,
    "execution_time_ms": 15
  }
}
```

## Monitoring Resource Usage

### Track Peak Memory

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/stress',
  args: ['--memory', '1', '--timeout', '1s'],
});

console.log('Peak memory:', result.stats?.memory_peak_bytes);
```

### Monitor CPU Time

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/stress',
  args: ['--cpu', '1', '--timeout', '1s'],
});

console.log('CPU time:', result.stats?.cpu_usage_usec, 'μs');
```

## Best Practices

1. **Monitor baseline usage** - Understand normal resource consumption
2. **Set appropriate limits** - Prevent resource exhaustion
3. **Track peak values** - Identify memory leaks or inefficiencies
4. **Use for optimization** - Compare resource usage across implementations

## See Also

- [Container Isolation](/core-concepts/isolation/overview/) - Security isolation
- [Task Execution](/core-concepts/execution/overview/) - Execution flow
