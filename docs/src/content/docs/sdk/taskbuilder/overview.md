---
title: Overview
description: Fluent API for building task sequences
---

# TaskBuilder

The TaskBuilder provides a fluent API for constructing complex task execution sequences.

## Basic Usage

```typescript
import { TaskBuilder } from '@faber/runtime-sdk';

const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['Step 1'] })
  .parallel([
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
  ])
  .single({ cmd: '/bin/echo', args: ['Final step'] });

const steps = builder.build();
const results = await client.executeGroup(builder);
```

## Methods

### `single(task: Task): this`

Add a single task to the sequence.

```typescript
builder.single({
  cmd: '/usr/bin/gcc',
  args: ['main.c', '-o', 'main'],
  files: {
    'main.c': '#include <stdio.h>\nint main() { printf("Hello"); return 0; }',
  },
});
```

### `parallel(tasks: Task[]): this`

Add multiple tasks to execute in parallel.

```typescript
builder.parallel([
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
]);
```

All tasks in the array run concurrently.

### `singleWithTests(task: Task, tests: TaskTest[]): this`

Add a single task with client-side tests.

```typescript
builder.singleWithTests(
  { cmd: '/bin/echo', args: ['Hello, World!'] },
  [
    {
      name: 'contains greeting',
      assertion: 'contains',
      field: 'stdout',
      expected: 'Hello',
    },
    {
      name: 'exit code 0',
      assertion: 'equals',
      field: 'exitCode',
      expected: 0,
    },
  ]
);
```

### `parallelWithTests(tasks: TaskWithTests[]): this`

Add parallel tasks with tests.

```typescript
builder.parallelWithTests([
  {
    cmd: '/bin/echo',
    args: ['Task 1'],
    tests: [
      { name: 'check output', assertion: 'contains', field: 'stdout', expected: 'Task 1' },
    ],
  },
  {
    cmd: '/bin/echo',
    args: ['Task 2'],
    tests: [
      { name: 'check output', assertion: 'contains', field: 'stdout', expected: 'Task 2' },
    ],
  },
]);
```

### `build(): ExecutionStep[]`

Build and return the execution steps array.

```typescript
const steps = builder.build();
// Returns: ExecutionStep[]
```

## Properties

### `length: number`

Get the number of steps in the builder.

```typescript
console.log(builder.length); // 3
```

### `isEmpty: boolean`

Check if the builder has no steps.

```typescript
if (builder.isEmpty) {
  console.log('No tasks added');
}
```

## Iterator

TaskBuilder is iterable:

```typescript
for (const step of builder) {
  console.log(step);
}
```

## Examples

### C Compilation Workflow

```typescript
const builder = new TaskBuilder()
  // Step 1: Compile
  .single({
    cmd: '/usr/bin/gcc',
    args: ['hello.c', '-o', 'hello'],
    files: {
      'hello.c': `
#include <stdio.h>
int main() {
    printf("Hello from C!\\n");
    return 0;
}
      `,
    },
  })
  // Step 2: Run
  .single({ cmd: './hello' });

const results = await client.executeGroup(builder);
console.log(results[1].stdout); // "Hello from C!\n"
```

### Parallel Testing

```typescript
const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['Starting tests...'] })
  .parallel([
    { cmd: '/bin/echo', args: ['Test 1 passed'] },
    { cmd: '/bin/echo', args: ['Test 2 passed'] },
    { cmd: '/bin/echo', args: ['Test 3 passed'] },
  ])
  .single({ cmd: '/bin/echo', args: ['All tests completed!'] });

const results = await client.executeGroup(builder);
// Total time: ~0ms (parallel execution)
```

### Data Processing Pipeline

```typescript
const builder = new TaskBuilder()
  // Fetch data
  .single({
    cmd: '/usr/bin/curl',
    args: ['-o', 'data.json', 'https://api.example.com/data'],
  })
  // Process in parallel
  .parallel([
    { cmd: '/usr/bin/jq', args: ['.users', 'data.json'] },
    { cmd: '/usr/bin/jq', args: ['.orders', 'data.json'] },
  ])
  // Generate report
  .single({ cmd: '/bin/echo', args: ['Report generated'] });

const results = await client.executeGroup(builder);
const users = results[1][0].stdout;
const orders = results[1][1].stdout;
```

## Chaining

All methods return `this`, enabling method chaining:

```typescript
const builder = new TaskBuilder()
  .single(task1)
  .parallel([task2, task3])
  .singleWithTests(task4, tests)
  .parallelWithTests([task5, task6]);
```

## Best Practices

1. **Use TaskBuilder for complex sequences** - More readable than raw arrays
2. **Group related tasks** - Use parallel execution for independent operations
3. **Add tests for validation** - Use `singleWithTests` and `parallelWithTests`
4. **Check `length` before execution** - Avoid empty task groups

## See Also

- [Testing Framework](/sdk/testing/overview/) - Add assertions to tasks
- [JavaScript SDK](/sdk/javascript/overview/) - SDK overview
- [Examples](/examples/patterns/basic/) - Common patterns
