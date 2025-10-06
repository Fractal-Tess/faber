# Faber JS SDK Integration Tests

This directory contains integration tests for the Faber JavaScript/TypeScript SDK.

## Overview

The integration tests verify that the SDK correctly communicates with a running Faber API server. These tests focus on:

- **C Program Compilation**: Testing compilation and execution of C programs
- **Compilation Output Verification**: Ensuring compiled programs produce expected output

## Prerequisites

Before running the integration tests, ensure:

1. **Faber API server is running**: The tests expect the server to be available
2. **Environment variables** (optional):
   - `FABER_BASE_URL`: Base URL for the API server (default: `http://localhost:3000`)
   - `FABER_API_KEY`: API key for authentication (default: `just-a-test-api-key`)

## Running the Tests

### Run all integration tests

```bash
npm run test:integration
```

### Run tests in watch mode (for development)

```bash
npm test -- --config test/integration-only.config.ts
```

### Run specific test file

```bash
npm test -- test/client.integration.test.ts
```

### Run with verbose output

```bash
npm test -- --config test/integration-only.config.ts --reporter=verbose
```

## Test Files

- **`c-compilation.integration.test.ts`**: Tests for C program compilation and execution

  - Basic C compilation (hello world)
  - C compilation with optimization flags
  - Multi-file C program compilation
  - C++ compilation and execution
  - Compilation error handling
  - Program output verification

## Test Configuration

The integration tests use the Vitest configuration defined in `vitest.config.ts`:

```typescript
{
  include: ['test/**/*.integration.test.ts'],
  testTimeout: 30000,
  environment: 'node',
}
```

## Troubleshooting

### Tests fail with connection errors

Ensure the Faber API server is running:

```bash
# Check if server is accessible
curl http://localhost:3000/api/v1/health
```

### Tests fail with authentication errors

Verify your API key is correct:

```bash
export FABER_API_KEY=your-actual-api-key
npm run test:integration
```

### Tests timeout

Increase the timeout in the Vitest config or for specific tests:

```typescript
it(
  'long running test',
  async () => {
    // test code
  },
  { timeout: 60000 }
);
```

## Notes

- **Server state**: The tests assume a clean server state for each test
- **Isolation**: Tests are designed to be independent and can run in parallel
- **No unit tests**: These are integration tests only; they require a running server
- **Read-only**: Tests do not modify server state in a way that affects other tests
