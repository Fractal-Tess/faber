---
title: Basic Patterns
description: Common usage patterns for Faber
---

# Common Patterns

Common patterns for using Faber effectively.

## Simple Command Execution

Execute a single command:

```typescript
const result = await client.executeSingle({
  cmd: '/bin/echo',
  args: ['Hello, World!'],
});

console.log(result.stdout); // "Hello, World!\n"
```

## Sequential Execution

Run tasks in sequence:

```typescript
const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['Step 1'] })
  .single({ cmd: '/bin/echo', args: ['Step 2'] })
  .single({ cmd: '/bin/echo', args: ['Step 3'] });

const results = await client.executeGroup(builder);
// Results: [result1, result2, result3]
```

Each step waits for the previous to complete.

## Parallel Execution

Run independent tasks concurrently:

```typescript
const builder = new TaskBuilder()
  .parallel([
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
  ]);

const results = await client.executeGroup(builder);
// Total time: ~1 second (not 3!)
```

## File Operations

### Create and Read Files

```typescript
const result = await client.executeSingle({
  cmd: '/bin/cat',
  args: ['hello.txt'],
  files: {
    'hello.txt': 'Hello from Faber!',
  },
});

console.log(result.stdout); // "Hello from Faber!"
```

### Multiple Files

```typescript
const result = await client.executeSingle({
  cmd: '/bin/ls',
  args: ['-la'],
  files: {
    'file1.txt': 'Content 1',
    'file2.txt': 'Content 2',
    'script.sh': '#!/bin/bash\necho "Hello"',
  },
});
```

## Environment Variables

Set environment for a task:

```typescript
const result = await client.executeSingle({
  cmd: '/bin/sh',
  args: ['-c', 'echo $GREETING from $USER'],
  env: {
    GREETING: 'Hello',
    USER: 'Faber',
  },
});

console.log(result.stdout); // "Hello from Faber\n"
```

## Standard Input

Provide input to commands:

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/python3',
  args: ['-c', 'name = input("Name: "); print(f"Hello, {name}!")'],
  stdin: 'Alice\n',
});

console.log(result.stdout); // "Name: Hello, Alice!\n"
```

## Working Directory

Set the working directory:

```typescript
const result = await client.executeSingle({
  cmd: '/bin/pwd',
  working_dir: '/tmp',
});

console.log(result.stdout); // "/tmp\n"
```

## Error Handling

Check exit codes:

```typescript
const result = await client.executeSingle({ cmd: '/bin/false' });

if (result.exitCode !== 0) {
  console.error('Command failed!');
  console.error('stderr:', result.stderr);
}
```

## Resource Monitoring

Access execution statistics:

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/stress',
  args: ['--cpu', '1', '--timeout', '1s'],
});

console.log('Peak memory:', result.stats?.memory_peak_bytes, 'bytes');
console.log('CPU usage:', result.stats?.cpu_usage_usec, 'μs');
console.log('Execution time:', result.stats?.execution_time_ms, 'ms');
console.log('Peak PIDs:', result.stats?.pids_peak);
```

## Health Check

Verify server status:

```typescript
try {
  const health = await client.health();
  console.log('Server is', health.status); // "ok"
} catch (error) {
  console.error('Server is down!');
}
```

## Chaining Results

Use output from one task in the next:

```typescript
// Not directly supported (stateless)
// Instead, combine into single task or use files

const builder = new TaskBuilder()
  .single({
    cmd: '/bin/sh',
    args: ['-c', 'echo "Hello" > /tmp/msg.txt && cat /tmp/msg.txt'],
  });

const results = await client.executeGroup(builder);
console.log(results[0].stdout); // "Hello\n"
```

## Batch Processing

Process multiple items:

```typescript
const items = ['item1', 'item2', 'item3'];

const builder = new TaskBuilder();
for (const item of items) {
  builder.single({
    cmd: '/bin/echo',
    args: [`Processing ${item}`],
  });
}

const results = await client.executeGroup(builder);
results.forEach((result, i) => {
  console.log(`${items[i]}: ${result.stdout}`);
});
```

## Timeout Pattern

Implement timeouts:

```typescript
const controller = new AbortController();
const timeout = setTimeout(() => controller.abort(), 5000);

try {
  const result = await client.executeSingle(
    { cmd: '/bin/sleep', args: ['10'] },
    { signal: controller.signal }
  );
} catch (error) {
  if (error.name === 'AbortError') {
    console.log('Task timed out');
  }
} finally {
  clearTimeout(timeout);
}
```

## Validation Pattern

Validate before execution:

```typescript
function validateTask(task: Task): boolean {
  if (!task.cmd || task.cmd.trim() === '') {
    throw new Error('Command is required');
  }
  if (task.args && !Array.isArray(task.args)) {
    throw new Error('Args must be an array');
  }
  return true;
}

const task = { cmd: '/bin/echo', args: ['Hello'] };
validateTask(task);
const result = await client.executeSingle(task);
```

## Next Steps

- [C/C++ Compilation](/examples/compilation/c/) - Compile and run C code
- [Python Scripts](/examples/python/basic/) - Run Python programs
- [Testing](/sdk/testing/overview/) - Add tests to your tasks
