# Faber JavaScript/TypeScript SDK

A JavaScript/TypeScript client SDK for the [Faber](https://github.com/Fractal-Tess/faber) secure task execution runtime. Execute commands in isolated, sandboxed containers with resource limits and monitoring.

## Features

- **Secure Execution**: Run commands in isolated Linux containers with namespace isolation
- **Resource Monitoring**: Track memory, CPU, and execution time for every task
- **Sequential & Parallel**: Execute tasks in sequence or run multiple tasks in parallel
- **Client-Side Testing**: Define tests on tasks to validate execution results
- **TypeScript First**: Full type safety with comprehensive type definitions
- **File Management**: Create files inline for your tasks to use
- **Environment Control**: Set environment variables and working directories

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

// Create a client
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!,
});

// Execute a simple command
const result = await client.executeSingle({
  cmd: '/bin/echo',
  args: ['Hello, World!'],
});

console.log(result.stdout); // "Hello, World!\n"
console.log(result.exitCode); // 0
console.log(result.stats); // Resource usage, terminal outcome, cgroup events, and cleanup status
```

## API Reference

### FaberClient

The main client for interacting with the Faber runtime API.

#### Constructor

```typescript
new FaberClient(config: FaberConfig)
```

**FaberConfig:**

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `baseUrl` | `string` | Yes | Base URL of the Faber server |
| `apiKey` | `string` | Yes | API key for authentication |
| `fetch` | `typeof fetch` | No | Custom fetch implementation (optional) |

```typescript
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: 'your-api-key',
});
```

#### Methods

##### `health(): Promise<HealthResponse>`

Check if the Faber server is running.

```typescript
const health = await client.health();
console.log(health.status); // "ok"
```

##### `executeSingle(task: Task): Promise<TaskResult>`

Execute a single task.

```typescript
const result = await client.executeSingle({
  cmd: '/bin/ls',
  args: ['-la'],
});

console.log(result.stdout);
console.log(result.stderr);
console.log(result.exitCode);
console.log(result.stats);
```

##### `executeGroup(steps: ExecutionStep[] | TaskBuilder): Promise<TaskGroupResult>`

Execute a group of tasks (sequential or parallel).

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

##### `executeWithTests(steps: ExecutionStep[] | TaskBuilder): Promise<ExecutionWithTestsResult>`

Execute tasks and run client-side tests to validate results.

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

### TaskBuilder

Fluent API for building task execution sequences.

```typescript
import { TaskBuilder } from '@faber/runtime-sdk';

const builder = new TaskBuilder();
```

#### Methods

##### `single(task: Task): this`

Add a single task to the execution sequence.

```typescript
builder.single({
  cmd: '/usr/bin/gcc',
  args: ['main.c', '-o', 'main'],
  files: {
    'main.c': '#include <stdio.h>\nint main() { printf("Hello"); return 0; }',
  },
});
```

##### `parallel(tasks: Task[]): this`

Add multiple tasks to be executed in parallel.

```typescript
builder.parallel([
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
  { cmd: '/bin/sleep', args: ['1'] },
]);
```

##### `singleWithTests(task: Task, tests: TaskTest[]): this`

Add a single task with client-side tests.

```typescript
builder.singleWithTests(
  { cmd: '/bin/echo', args: ['Hello, World!'] },
  [
    { name: 'contains greeting', assertion: 'contains', field: 'stdout', expected: 'Hello' },
    { name: 'exit code 0', assertion: 'equals', field: 'exitCode', expected: 0 },
  ]
);
```

##### `parallelWithTests(tasks: TaskWithTests[]): this`

Add parallel tasks with tests.

```typescript
builder.parallelWithTests([
  {
    cmd: '/bin/echo',
    args: ['Hello'],
    tests: [{ name: 'check output', assertion: 'contains', field: 'stdout', expected: 'Hello' }],
  },
  {
    cmd: '/bin/echo',
    args: ['World'],
    tests: [{ name: 'check output', assertion: 'contains', field: 'stdout', expected: 'World' }],
  },
]);
```

##### `build(): ExecutionStep[]`

Build and return the execution steps array.

```typescript
const steps = builder.build();
```

##### Properties

- `length: number` - Number of steps in the builder
- `isEmpty: boolean` - Whether the builder has no steps

### Testing

The SDK includes a client-side testing framework for validating task execution results.

#### Test Types

**Equals Test:**

```typescript
{
  name: 'exit code check',
  assertion: 'equals',
  field: 'exitCode', // 'stdout' | 'stderr' | 'exitCode'
  expected: 0,
}
```

**Contains Test:**

```typescript
{
  name: 'stdout check',
  assertion: 'contains',
  field: 'stdout', // 'stdout' | 'stderr'
  expected: 'success',
}
```

**Matches Test (RegExp):**

```typescript
{
  name: 'pattern check',
  assertion: 'matches',
  field: 'stdout',
  expected: /Hello, \w+!/,
}
```

**Custom Test:**

```typescript
{
  name: 'custom validation',
  assertion: 'custom',
  testFn: (result: TaskResult) => ({
    name: 'custom validation',
    passed: result.exitCode === 0 && result.stdout.length > 0,
    message: 'Custom validation passed',
  }),
}
```

### TestResultAnalyzer

Analyze and report on test execution results.

```typescript
import { TestResultAnalyzer } from '@faber/runtime-sdk';

const result = await client.executeWithTests(builder);
const analyzer = new TestResultAnalyzer(result);

// Check if all tests passed
console.log(analyzer.allPassed); // boolean

// Get summary counts
console.log(analyzer.totalSteps);
console.log(analyzer.passedSteps);
console.log(analyzer.failedSteps);

// Get detailed failure information
const failures = analyzer.getFailedSteps();
const firstFailure = analyzer.getFirstFailure();

// Generate formatted report
console.log(analyzer.formatReport({ showPassed: true, includeOutput: true }));

// Assert all tests passed (throws if any failed)
analyzer.assertAllPassed('Custom error message');
```

## Examples

### Working with Files

Create files that your tasks can use:

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/python3',
  args: ['script.py'],
  files: {
    'script.py': `
import json

data = {"message": "Hello from Python"}
print(json.dumps(data))
    `,
  },
});

console.log(result.stdout); // {"message": "Hello from Python"}
```

### Environment Variables

Set environment variables for your tasks:

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

### Standard Input

Provide stdin to your commands:

```typescript
const result = await client.executeSingle({
  cmd: '/usr/bin/python3',
  args: ['-c', 'name = input("Name: "); print(f"Hello, {name}!")'],
  stdin: 'Alice\n',
});

console.log(result.stdout); // "Name: Hello, Alice!\n"
```

### Working Directory

Set the working directory for execution:

```typescript
const result = await client.executeSingle({
  cmd: '/bin/pwd',
  working_dir: '/tmp',
});

console.log(result.stdout); // "/tmp\n"
```

### C Compilation Example

Compile and run a C program:

```typescript
import { FaberClient, TaskBuilder } from '@faber/runtime-sdk';

const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!,
});

const builder = new TaskBuilder()
  // Step 1: Compile the C program
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
  // Step 2: Run the compiled program
  .single({
    cmd: './hello',
  });

const results = await client.executeGroup(builder);

console.log(results[0].exitCode); // 0 (compilation success)
console.log(results[1].stdout);   // "Hello from C!\n"
```

### Parallel Execution

Run multiple tasks in parallel:

```typescript
const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['Starting parallel tasks...'] })
  .parallel([
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
  ])
  .single({ cmd: '/bin/echo', args: ['All parallel tasks completed!'] });

const results = await client.executeGroup(builder);
// Total execution time: ~1 second (not 3 seconds!)
```

### Testing Workflow

Complete example with client-side testing:

```typescript
import { FaberClient, TaskBuilder, TestResultAnalyzer } from '@faber/runtime-sdk';

const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!,
});

const builder = new TaskBuilder()
  .singleWithTests(
    {
      cmd: '/bin/echo',
      args: ['Hello, World!'],
    },
    [
      {
        name: 'stdout contains greeting',
        assertion: 'contains',
        field: 'stdout',
        expected: 'Hello',
      },
      {
        name: 'exit code is 0',
        assertion: 'equals',
        field: 'exitCode',
        expected: 0,
      },
    ]
  )
  .parallelWithTests([
    {
      cmd: '/bin/echo',
      args: ['Task 1'],
      tests: [
        {
          name: 'task 1 output',
          assertion: 'matches',
          field: 'stdout',
          expected: /Task \d+/,
        },
      ],
    },
    {
      cmd: '/bin/echo',
      args: ['Task 2'],
      tests: [
        {
          name: 'task 2 output',
          assertion: 'contains',
          field: 'stdout',
          expected: 'Task 2',
        },
      ],
    },
  ]);

const result = await client.executeWithTests(builder);
const analyzer = new TestResultAnalyzer(result);

// Print detailed report
console.log(analyzer.formatReport({ showPassed: true }));

// Assert all tests passed
analyzer.assertAllPassed();
```

## Types

### Task

```typescript
type Task = {
  cmd: string;                    // Command to execute
  args?: string[];                // Command arguments
  env?: Record<string, string>;   // Environment variables
  stdin?: string;                 // Standard input
  files?: Record<string, string>; // Files to create
  working_dir?: string;           // Working directory
  sandbox_profile?: 'compile_v1' | 'native_v1'; // Seccomp policy
};
```

### TaskResult

```typescript
type TaskResult = {
  stdout: string;      // Standard output
  stderr: string;      // Standard error
  exitCode: number;    // Exit code (0 = success)
  stats?: {
    memory_peak_bytes: number;    // Peak memory usage
    cpu_usage_usec: number;       // CPU usage in microseconds
    cpu_nr_throttled: number;     // Number of throttled periods
    cpu_throttled_usec: number;   // Total throttled time
    pids_peak: number;            // Peak process count
    execution_time_ms: number;    // Execution time in milliseconds
    stdout_truncated: boolean;    // Stdout exceeded the configured limit
    stderr_truncated: boolean;    // Stderr exceeded the configured limit
    outcome: TaskOutcome;         // Explicit terminal outcome
    termination_signal: number | null;
    oom_kill_count: number;       // memory.events evidence
    pids_limit_hit_count: number; // pids.events evidence
    cleanup_succeeded: boolean;
  };
};
```

### ExecutionStep

```typescript
type ExecutionStep = Task | Task[];  // Single task or parallel tasks
```

### TaskGroupResult

```typescript
type TaskGroupResult = (TaskResult | TaskResult[])[];
```

## Error Handling

The SDK throws errors for API failures:

```typescript
try {
  const result = await client.executeSingle({ cmd: '/bin/false' });
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Configuration error:', error.message);
  } else {
    console.error('API error:', error.message);
  }
}
```

Note: A non-zero exit code from a command is **not** an error - it's a valid result. Check `result.exitCode` to determine success or failure.

## License

MIT

---

For more information, visit the [Faber GitHub repository](https://github.com/Fractal-Tess/faber).
