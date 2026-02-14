---
title: Task Execution
description: Understanding how tasks are executed
---

# Task Execution

How Faber executes tasks from request to completion.

## Execution Flow

```
1. API Request
   ↓
2. Authentication
   ↓
3. Cache Check
   ↓
4. Runtime Creation
   ↓
5. Container Setup
   ↓
6. Task Execution
   ↓
7. Stats Collection
   ↓
8. Cleanup
   ↓
9. Response
```

## Step-by-Step

### 1. API Request

Client sends POST request to `/api/v1/execute` with task group.

### 2. Authentication

Middleware validates API key from Authorization header.

### 3. Cache Check

If caching is enabled, check for existing result.

### 4. Runtime Creation

RuntimeBuilder creates a Runtime instance with:
- Task group
- Cgroup configuration
- Container configuration

### 5. Container Setup

Fork process and setup:
- Namespaces (PID, mount, network, UTS, IPC)
- pivot_root for filesystem isolation
- Cgroup assignment
- Capability dropping
- Unprivileged user

### 6. Task Execution

For each execution step:
- **Single task**: Fork → exec → wait
- **Parallel tasks**: Fork all → wait all

### 7. Stats Collection

Read cgroup statistics:
- memory.peak
- cpu.stat
- pids.peak

### 8. Cleanup

- Kill remaining processes
- Remove cgroup directory
- Unmount container filesystem

### 9. Response

Return results with statistics to client.

## Parallel Execution

When a step contains an array of tasks:

1. All tasks are forked simultaneously
2. Each runs in its own process
3. Parent waits for all children
4. Results are collected in order

```typescript
// These run in parallel
[
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
]
// Total time: ~1 second
```

## Sequential Execution

When steps are separate elements:

1. Step 1 executes
2. Wait for completion
3. Step 2 executes
4. Wait for completion

```typescript
// These run sequentially
[
  { cmd: '/bin/echo', args: ['Step 1'] },
  { cmd: '/bin/echo', args: ['Step 2'] },
]
// Total time: step1 + step2
```

## Error Handling

### Container Setup Failure

Returns HTTP 500 with error logged.

### Task Execution Failure

Non-zero exit code is valid result, not error.

### Resource Limit Exceeded

Process is killed by cgroup, result shows error.

## See Also

- [Container Isolation](/core-concepts/isolation/overview/) - Security layers
- [Resource Management](/core-concepts/resources/overview/) - Resource limits
