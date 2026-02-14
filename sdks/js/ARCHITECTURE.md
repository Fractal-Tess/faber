# SDK Architecture

This document describes the architecture and organization of the Faber SDK.

## Directory Structure

```
src/
├── client/                 # API client implementation
│   ├── faber-client.ts    # Main client class
│   └── index.ts           # Barrel export
│
├── models/                 # Domain models and business logic
│   ├── task.ts            # Task type definitions
│   ├── task-group.ts      # TaskGroup class
│   └── index.ts           # Barrel export
│
├── types/                  # Type definitions
│   ├── config.ts          # Configuration types
│   ├── execution.ts       # Execution-related types
│   ├── test.ts            # Test-related types
│   └── index.ts           # Barrel export
│
├── errors/                 # Error classes
│   ├── faber-error.ts     # Base error class
│   ├── connection-error.ts
│   ├── timeout-error.ts
│   ├── validation-error.ts
│   ├── execution-error.ts
│   ├── api-error.ts
│   └── index.ts           # Barrel export
│
├── utils/                  # Utility functions
│   ├── array.ts           # Array utilities
│   └── index.ts           # Barrel export
│
└── index.ts               # Main SDK export
```

## Design Principles

### 1. Separation of Concerns

- **Client**: Handles HTTP communication with the Faber API
- **Models**: Contains domain logic (TaskGroup, Task)
- **Types**: Pure TypeScript type definitions
- **Errors**: Custom error classes for better error handling
- **Utils**: Reusable utility functions

### 2. Barrel Exports

Each directory has an `index.ts` that serves as a barrel export, providing:

- Clean import paths
- Single source of truth for module exports
- Easy refactoring and maintenance

### 3. TypeScript Best Practices

- **Interface over Type**: Use `interface` for object shapes that can be extended
- **Type for Unions/Aliases**: Use `type` for unions, mapped types, and utility types
- **Readonly Properties**: Mark properties as `readonly` where appropriate
- **Proper Access Modifiers**: Use `private`, `protected`, `public` explicitly

### 4. Naming Conventions

- **Files**: kebab-case (e.g., `faber-client.ts`, `task-group.ts`)
- **Classes**: PascalCase (e.g., `FaberClient`, `TaskGroup`)
- **Interfaces/Types**: PascalCase (e.g., `Task`, `FaberConfig`)
- **Functions/Variables**: camelCase (e.g., `execute`, `normalizeResponse`)
- **Constants**: UPPER_SNAKE_CASE (if any)

## Module Dependencies

```
index.ts
  ├── client/
  │   └── errors/ (for ValidationError)
  │   └── types/ (for FaberConfig, TaskResult, etc.)
  │   └── models/ (for ExecutionStep)
  │
  ├── models/
  │   └── client/ (for FaberClient reference)
  │   └── types/ (for TaskResult, TestResult, TestFunction)
  │   └── utils/ (for zip function)
  │
  ├── types/
  │   └── (no dependencies - pure types)
  │
  ├── errors/
  │   └── (no dependencies - pure classes)
  │
  └── utils/
      └── (no dependencies - pure functions)
```

## Key Features

### 1. Type Safety

All public APIs are fully typed with TypeScript, providing:

- Autocomplete in IDEs
- Compile-time error checking
- Better documentation through types

### 2. Error Handling

Custom error classes inherit from `FaberError`:

- `ConnectionError`: Network/connection issues
- `TimeoutError`: Request timeouts
- `ValidationError`: Input validation failures
- `ExecutionError`: Task execution failures
- `ApiError`: API response errors

### 3. Fluent API

TaskGroup uses method chaining for a clean, readable API:

```typescript
taskGroup
  .single({ cmd: 'echo', args: ['hello'] })
  .parallel([{ cmd: 'ls', args: ['-la'] }, { cmd: 'pwd' }])
  .execute();
```

### 4. Extensibility

The architecture supports easy extension:

- Add new error types in `errors/`
- Add new utility functions in `utils/`
- Add new models in `models/`
- Add new types in `types/`

## Usage

### Basic Import

```typescript
import { FaberClient, TaskGroup } from '@faber/sdk';
```

### Type Imports

```typescript
import type { Task, FaberConfig, TestResult } from '@faber/sdk';
```

### Error Handling

```typescript
import { FaberClient, ValidationError, ApiError } from '@faber/sdk';

try {
  const client = new FaberClient(config);
} catch (error) {
  if (error instanceof ValidationError) {
    // Handle validation error
  } else if (error instanceof ApiError) {
    // Handle API error
  }
}
```

## Testing Considerations

The modular structure makes testing easier:

- **Unit Tests**: Test individual modules in isolation
- **Integration Tests**: Test client communication
- **E2E Tests**: Test full workflows

Each module can be mocked independently for testing.

## Future Enhancements

Possible improvements while maintaining the architecture:

1. Add retry logic in the client
2. Add request/response interceptors
3. Add streaming support for long-running tasks
4. Add metrics and observability hooks
5. Add caching layer for repeated requests

