---
title: Overview
description: JavaScript/TypeScript SDK for Faber
---

# JavaScript SDK

The Faber JavaScript/TypeScript SDK provides a convenient client for the Faber API.

## Installation

### npm

```bash
npm install @faber/runtime-sdk
```

### yarn

```bash
yarn add @faber/runtime-sdk
```

### bun

```bash
bun add @faber/runtime-sdk
```

## Quick Start

```typescript
import { FaberClient, TaskBuilder } from '@faber/runtime-sdk';

// Create client
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!,
});

// Execute a simple task
const result = await client.executeSingle({
  cmd: '/bin/echo',
  args: ['Hello, World!'],
});

console.log(result.stdout); // "Hello, World!\n"
console.log(result.exitCode); // 0
console.log(result.stats); // Resource statistics
```

## FaberClient

The main client class for interacting with the Faber API.

### Constructor

```typescript
new FaberClient(config: FaberConfig)
```

**FaberConfig:**

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `baseUrl` | `string` | Yes | Base URL of the Faber server |
| `apiKey` | `string` | Yes | API key for authentication |
| `fetch` | `typeof fetch` | No | Custom fetch implementation |

### Methods

#### `health(): Promise<HealthResponse>`

Check server health.

```typescript
const health = await client.health();
console.log(health.status); // "ok"
```

#### `executeSingle(task: Task): Promise<TaskResult>`

Execute a single task.

```typescript
const result = await client.executeSingle({
  cmd: '/bin/ls',
  args: ['-la'],
});
```

#### `executeGroup(steps: ExecutionStep[] | TaskBuilder): Promise<TaskGroupResult>`

Execute a group of tasks.

```typescript
// Using TaskBuilder
const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['step 1'] })
  .single({ cmd: '/bin/echo', args: ['step 2'] });

const results = await client.executeGroup(builder);

// Or with raw steps
const results = await client.executeGroup([
  { cmd: '/bin/echo', args: ['step 1'] },
  { cmd: '/bin/echo', args: ['step 2'] },
]);
```

#### `executeWithTests(steps: ExecutionStep[] | TaskBuilder): Promise<ExecutionWithTestsResult>`

Execute tasks and run client-side tests.

```typescript
const builder = new TaskBuilder()
  .singleWithTests(
    { cmd: '/bin/echo', args: ['Hello'] },
    [
      { name: 'check stdout', assertion: 'contains', field: 'stdout', expected: 'Hello' },
      { name: 'check exit code', assertion: 'equals', field: 'exitCode', expected: 0 },
    ]
  );

const result = await client.executeWithTests(builder);
console.log(result.allTestsPassed); // true or false
```

## Type Definitions

### Task

```typescript
type Task = {
  cmd: string;
  args?: string[];
  env?: Record<string, string>;
  stdin?: string;
  files?: Record<string, string>;
  working_dir?: string;
  sandbox_profile?: 'compile_v1' | 'native_v1';
};
```

### TaskResult

```typescript
type TaskResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  stats?: {
    memory_peak_bytes: number;
    cpu_usage_usec: number;
    pids_peak: number;
    execution_time_ms: number;
  };
};
```

### ExecutionStep

```typescript
type ExecutionStep = Task | Task[];
```

## Error Handling

The SDK throws errors for API failures:

```typescript
import { ValidationError, ConnectionError } from '@faber/runtime-sdk';

try {
  const result = await client.executeSingle({ cmd: '/bin/false' });
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Configuration error:', error.message);
  } else if (error instanceof ConnectionError) {
    console.error('Connection error:', error.message);
  } else {
    console.error('API error:', error.message);
  }
}
```

Note: A non-zero exit code is **not** an error - it's a valid result. Check `result.exitCode` to determine success or failure.

## Next Steps

- [TaskBuilder Guide](/sdk/taskbuilder/overview/) - Build complex task sequences
- [Testing Framework](/sdk/testing/overview/) - Add client-side tests
- [Examples](/examples/patterns/basic/) - Common usage patterns
