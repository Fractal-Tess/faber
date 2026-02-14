# Faber JavaScript SDK

**Package**: `@faber/runtime-sdk`  
**Purpose**: TypeScript client for Faber task execution API with client-side testing

---

## Overview

The JavaScript/TypeScript SDK provides a complete client for the Faber API with:

- **FaberClient**: HTTP client with authentication
- **TaskBuilder**: Fluent API for building task sequences
- **Test Framework**: Client-side assertions on task results
- **Type Safety**: Full TypeScript definitions

---

## Structure

```
sdks/js/
├── src/
│   ├── index.ts              # Main exports
│   ├── client/
│   │   └── faber-client.ts   # FaberClient class
│   ├── builders/
│   │   └── task-builder.ts   # TaskBuilder fluent API
│   ├── models/
│   │   └── task.ts           # Task, ExecutionStep, TaskGroup
│   ├── types/
│   │   ├── index.ts          # Core types
│   │   └── tests.ts          # Test-related types
│   ├── utils/
│   │   ├── test-runner.ts    # Test execution
│   │   ├── test-result-analyzer.ts  # Result analysis
│   │   └── task-utils.ts     # Task manipulation
│   └── errors/
│       └── index.ts          # Error classes
│
├── test/
│   ├── *.test.ts             # Unit tests (106 tests)
│   └── *.integration.test.ts # Integration tests (30 tests)
│
├── package.json              # @faber/runtime-sdk v0.2.0
├── tsconfig.json             # TypeScript config
├── tsup.config.ts            # Build config (CJS/ESM/IIFE)
└── vitest.config.ts          # Test config
```

---

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| API client | `src/client/faber-client.ts` | HTTP methods, auth |
| Task builder | `src/builders/task-builder.ts` | Fluent API |
| Type definitions | `src/types/` | Core + test types |
| Test runner | `src/utils/test-runner.ts` | Assertion logic |
| Result analysis | `src/utils/test-result-analyzer.ts` | Reporting |
| Error types | `src/errors/index.ts` | Error hierarchy |
| Unit tests | `test/*.test.ts` | Vitest tests |
| Integration | `test/*.integration.test.ts` | API tests |

---

## Key Patterns

### FaberClient Usage
```typescript
const client = new FaberClient({
  baseUrl: 'http://localhost:3000',
  apiKey: process.env.FABER_API_KEY!,
});

// Simple execution
const result = await client.executeSingle({
  cmd: '/bin/echo',
  args: ['Hello'],
});

// With tests
const result = await client.executeWithTests(builder);
```

### TaskBuilder Pattern
```typescript
const builder = new TaskBuilder()
  .single({ cmd: '/bin/echo', args: ['step 1'] })
  .parallel([
    { cmd: '/bin/sleep', args: ['1'] },
    { cmd: '/bin/sleep', args: ['1'] },
  ])
  .singleWithTests(
    { cmd: '/bin/echo', args: ['final'] },
    [{ name: 'check', assertion: 'equals', field: 'exitCode', expected: 0 }]
  );

const results = await client.executeGroup(builder);
```

### Test Definitions
```typescript
// Equals test
{ name: 'exit code', assertion: 'equals', field: 'exitCode', expected: 0 }

// Contains test
{ name: 'output', assertion: 'contains', field: 'stdout', expected: 'Hello' }

// Matches test (regex)
{ name: 'pattern', assertion: 'matches', field: 'stdout', expected: /Task \d+/ }

// Custom test
{
  name: 'custom',
  assertion: 'custom',
  testFn: (result) => ({ passed: result.exitCode === 0, message: 'ok' })
}
```

---

## Conventions

### TypeScript
- **Type aliases only** — never interfaces (project preference)
- **Explicit exports** — no default exports
- **Naming**: `PascalCase` types, `camelCase` functions
- **File organization**: By feature (client/, builders/, types/, utils/)

### Testing
- **Unit tests**: `*.test.ts` — fast, isolated
- **Integration tests**: `*.integration.test.ts` — require running API
- **Timeout**: 30 seconds for integration tests
- **Framework**: Vitest with jsdom environment

### Error Handling
```typescript
// SDK errors
FaberError           // Base class
ConnectionError      // Network issues
TimeoutError         // Request timeout
ValidationError      // Invalid input
ExecutionError       // API returned error
ApiError             // HTTP error response
```

---

## Build System

```bash
# Development
npm run dev              # Watch mode with tsup

# Build
npm run build            # CJS + ESM + IIFE outputs
npm run type-check       # tsc --noEmit

# Testing
npm run test             # Unit tests
npm run test:integration # Integration tests

# Release
npx changeset version    # Bump version, update changelog
npm run build
git add . && git commit -m "chore: release vX.Y.Z"
```

**Build outputs:**
- `dist/index.js` — CJS
- `dist/index.mjs` — ESM
- `dist/index.iife.js` — IIFE (browser)
- `dist/index.d.ts` — TypeScript declarations

---

## Release Workflow

Uses **changesets** for versioning:

1. Make changes
2. `npx changeset` — create changeset file
3. `npx changeset version` — bump version, update CHANGELOG.md
4. `npm run build`
5. `git add . && git commit -m "chore: release"`
6. `git push`

**Note**: Separate changeset config from root (monorepo pattern).

---

## Anti-Patterns

**NEVER:**
- Use `as any` or `@ts-ignore` (strict types enforced)
- Use interfaces instead of type aliases
- Send tests to the API (tests are client-side only)
- Skip tests before committing changes

**ALWAYS:**
- Use TaskBuilder for complex sequences
- Handle errors with specific error classes
- Clean up test resources
- Run `npm run type-check` before commit
