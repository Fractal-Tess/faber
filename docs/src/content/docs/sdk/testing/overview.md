---
title: Overview
description: Client-side testing framework for task results
---

# Testing Framework

The Faber SDK includes a client-side testing framework for validating task execution results.

## Overview

Tests are defined alongside tasks and executed after the task completes. They allow you to assert conditions on:

- `stdout` - Standard output
- `stderr` - Standard error  
- `exitCode` - Exit code
- Custom validation functions

## Test Types

### Equals Test

Check exact equality:

```typescript
{
  name: 'exit code check',
  assertion: 'equals',
  field: 'exitCode',
  expected: 0,
}
```

### Contains Test

Check if output contains a substring:

```typescript
{
  name: 'stdout check',
  assertion: 'contains',
  field: 'stdout',
  expected: 'success',
}
```

### Matches Test

Check with regular expression:

```typescript
{
  name: 'pattern check',
  assertion: 'matches',
  field: 'stdout',
  expected: /Hello, \w+!/,
}
```

### Custom Test

Define custom validation logic:

```typescript
{
  name: 'custom validation',
  assertion: 'custom',
  testFn: (result: TaskResult) => ({
    passed: result.exitCode === 0 && result.stdout.length > 0,
    message: 'Exit code 0 and non-empty output',
  }),
}
```

## Using Tests

### With TaskBuilder

```typescript
import { TaskBuilder } from '@faber/runtime-sdk';

const builder = new TaskBuilder()
  .singleWithTests(
    { cmd: '/bin/echo', args: ['Hello, World!'] },
    [
      {
        name: 'contains greeting',
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
  );

const result = await client.executeWithTests(builder);
console.log(result.allTestsPassed); // true or false
```

### Parallel Tasks with Tests

```typescript
const builder = new TaskBuilder()
  .parallelWithTests([
    {
      cmd: '/bin/echo',
      args: ['Task 1'],
      tests: [
        { name: 'task 1 output', assertion: 'contains', field: 'stdout', expected: 'Task 1' },
      ],
    },
    {
      cmd: '/bin/echo',
      args: ['Task 2'],
      tests: [
        { name: 'task 2 output', assertion: 'contains', field: 'stdout', expected: 'Task 2' },
      ],
    },
  ]);

const result = await client.executeWithTests(builder);
```

## TestResultAnalyzer

Analyze and report on test execution results.

```typescript
import { TestResultAnalyzer } from '@faber/runtime-sdk';

const result = await client.executeWithTests(builder);
const analyzer = new TestResultAnalyzer(result);

// Check if all tests passed
console.log(analyzer.allPassed); // boolean

// Get summary counts
console.log(analyzer.totalSteps); // Total number of steps
console.log(analyzer.passedSteps); // Steps with all tests passed
console.log(analyzer.failedSteps); // Steps with any failed tests

// Get failure details
const failures = analyzer.getFailedSteps();
const firstFailure = analyzer.getFirstFailure();

// Generate formatted report
console.log(analyzer.formatReport({ showPassed: true, includeOutput: true }));

// Assert all tests passed (throws if any failed)
analyzer.assertAllPassed('Custom error message');
```

## ExecutionWithTestsResult

The result type from `executeWithTests`:

```typescript
type ExecutionWithTestsResult = {
  results: TaskGroupResult;
  stepResults: StepWithTestsResult[];
  allTestsPassed: boolean;
  passedCount: number;
  failedCount: number;
};
```

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `results` | `TaskGroupResult` | Raw task results |
| `stepResults` | `StepWithTestsResult[]` | Results with tests per step |
| `allTestsPassed` | `boolean` | Whether all tests passed |
| `passedCount` | `number` | Number of passed steps |
| `failedCount` | `number` | Number of failed steps |

## Examples

### CI/CD Validation

```typescript
const builder = new TaskBuilder()
  .singleWithTests(
    {
      cmd: '/usr/bin/npm',
      args: ['test'],
    },
    [
      { name: 'tests pass', assertion: 'equals', field: 'exitCode', expected: 0 },
    ]
  )
  .singleWithTests(
    {
      cmd: '/usr/bin/npm',
      args: ['run', 'build'],
    },
    [
      { name: 'build succeeds', assertion: 'equals', field: 'exitCode', expected: 0 },
      { name: 'no errors', assertion: 'equals', field: 'stderr', expected: '' },
    ]
  );

const result = await client.executeWithTests(builder);
const analyzer = new TestResultAnalyzer(result);
analyzer.assertAllPassed('Build validation failed');
```

### Output Validation

```typescript
const builder = new TaskBuilder()
  .singleWithTests(
    {
      cmd: '/usr/bin/python3',
      args: ['-c', 'print("Result: 42")'],
    },
    [
      { name: 'has result', assertion: 'contains', field: 'stdout', expected: 'Result:' },
      { name: 'correct value', assertion: 'matches', field: 'stdout', expected: /Result: \d+/ },
    ]
  );

const result = await client.executeWithTests(builder);
console.log(result.allTestsPassed); // true
```

### Complex Validation

```typescript
const builder = new TaskBuilder()
  .singleWithTests(
    {
      cmd: '/usr/bin/curl',
      args: ['-s', 'https://api.example.com/health'],
    },
    [
      {
        name: 'valid json response',
        assertion: 'custom',
        testFn: (result) => {
          try {
            const data = JSON.parse(result.stdout);
            return {
              passed: data.status === 'healthy',
              message: `API status: ${data.status}`,
            };
          } catch {
            return { passed: false, message: 'Invalid JSON' };
          }
        },
      },
    ]
  );
```

## Best Practices

1. **Name your tests clearly** - Makes failures easier to understand
2. **Test one thing per test** - Easier to identify failures
3. **Use appropriate assertion type**:
   - `equals` for exact matches
   - `contains` for substring checks
   - `matches` for pattern matching
   - `custom` for complex logic
4. **Always check `allTestsPassed`** - Don't assume success
5. **Use TestResultAnalyzer** - For detailed reporting

## See Also

- [TaskBuilder](/sdk/taskbuilder/overview/) - Build task sequences with tests
- [JavaScript SDK](/sdk/javascript/overview/) - SDK overview
