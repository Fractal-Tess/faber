# Faber JS SDK Examples

This directory contains practical examples demonstrating how to use the Faber JavaScript/TypeScript SDK for various task execution scenarios, with and without the TaskWithTest functionality.

## Examples Overview

### 1. GNU Commands (`gnu-commands.ts`)
Demonstrates basic command-line operations using standard GNU/Linux utilities.

**Features shown:**
- Simple command execution without tests
- Command execution with TaskWithTest validation
- Multi-step workflows with file operations
- Test validation for command outputs

**Key commands demonstrated:**
- `echo` - Text output
- `ls` - Directory listing
- `sh` - Shell operations
- `cat` - File content display
- `rm` - File deletion
- `wc` - Word counting
- `grep` - Pattern searching

### 2. C Compilation (`c-compilation.ts`)
Comprehensive examples of C program compilation and testing.

**Features shown:**
- Basic C compilation without tests
- C compilation with TaskWithTest validation
- Compilation error detection and testing
- Mathematical function testing
- TaskBuilder integration with C programs
- Complex calculation validation

**Programs demonstrated:**
- Hello World program
- Mathematical operations (factorial, square root, power)
- Calculator with arithmetic operations
- Sorting algorithms
- Error handling scenarios

### 3. Advanced Testing (`advanced-testing.ts`)
Sophisticated testing scenarios including performance and parallel execution.

**Features shown:**
- Performance testing with execution time limits
- Memory usage validation
- Error condition testing
- Parallel compilation and execution
- Complex multi-file project workflows
- TaskBuilder for complex project builds

**Scenarios demonstrated:**
- CPU-intensive program testing
- Compilation error detection
- Parallel execution of multiple programs
- Multi-file C project building
- End-to-end workflow validation

## Running the Examples

### Prerequisites
1. **Faber Runtime Server**: Make sure a Faber dev server is running on `http://localhost:3000`
2. **API Key**: The examples are configured to use `just-a-test-api-key` (dev container default)
3. **Development Environment**: Install dependencies with `bun install` or `npm install`

### Starting the Dev Server
```bash
# From the project root directory
docker-compose -f docker/dev/docker-compose.yaml up -d

# Or run with cargo (if you have the required dependencies)
cargo run
```

### Setup
```bash
cd /path/to/faber/sdks/js
bun install
```

### Running Individual Examples
```bash
# GNU Commands example
bun run examples/gnu-commands.ts

# C Compilation example
bun run examples/c-compilation.ts

# Advanced Testing example
bun run examples/advanced-testing.ts
```

### Running as TypeScript (for development)
```bash
# Using ts-node (if installed)
npx ts-node examples/gnu-commands.ts
npx ts-node examples/c-compilation.ts
npx ts-node examples/advanced-testing.ts
```

## Configuration

Update the client configuration in each example file to match your setup:

```typescript
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',           // Your Faber server URL
  apiKey: 'just-a-test-api-key'               // Dev container API key
});
```

Or use environment variables:

```typescript
const client = new FaberClient({
  baseUrl: process.env.FABER_BASE_URL || 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY || 'just-a-test-api-key'
});
```

## Key Concepts Demonstrated

### TaskWithTest Usage
```typescript
const taskWithTest: TaskWithTest = {
  cmd: 'gcc',
  args: ['program.c', '-o', 'program'],
  files: { 'program.c': '...' },
  test: (context: TestContext): TestResult => {
    return {
      passed: context.exit_code === 0,
      message: context.exit_code === 0 ? 'Success' : 'Failed',
      details: { exit_code: context.exit_code }
    };
  }
};
```

### TaskBuilder Integration
```typescript
const taskBuilder = new TaskBuilder()
  .singleWithTest(task, testFunction)
  .singleWithTest(anotherTask, anotherTestFunction);

const result = await client.executeGroupWithTests(taskBuilder);
```

### Test Result Validation
```typescript
if (result.allTestsPassed) {
  console.log('✅ All tests passed!');
} else {
  console.log('❌ Some tests failed:');
  result.failedTests.forEach(test => {
    console.log(`- ${test.message}`);
  });
}
```

## Expected Output

Each example will produce detailed output showing:
- Task execution results
- Test validation outcomes
- Performance metrics
- Error handling (when applicable)
- Detailed test results with pass/fail status

## Troubleshooting

### Connection Issues
- Ensure the Faber server is running on the specified port
- Check that your API key is correct
- Verify network connectivity to the server

### Compilation Issues
- Make sure GCC is available in the Faber container
- Check that the container has sufficient permissions
- Verify file paths and permissions

### Test Failures
- Review test logic and expected values
- Check that programs produce the expected output format
- Verify that performance thresholds are reasonable

## Learning Path

1. **Start with `gnu-commands.ts`** - Basic command execution and simple tests
2. **Move to `c-compilation.ts`** - Compilation workflows and output validation
3. **Explore `advanced-testing.ts`** - Complex scenarios and performance testing

Each example builds on concepts from the previous ones, providing a comprehensive understanding of the SDK's capabilities.