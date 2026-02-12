# Module Dependency Diagram

This document visualizes the module structure and dependencies in the Faber SDK.

## High-Level Architecture

```
┌─────────────────────────────────────────────────┐
│                   index.ts                      │
│            (Main Package Export)                │
└────────────┬────────────────────────────────────┘
             │
             │ exports from
             │
    ┌────────┴────────┬─────────┬─────────┬───────┐
    │                 │         │         │       │
    ▼                 ▼         ▼         ▼       ▼
┌─────────┐    ┌──────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│ client/ │    │ models/  │ │ types/ │ │errors/ │ │ utils/ │
└─────────┘    └──────────┘ └────────┘ └────────┘ └────────┘
```

## Module Dependencies

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
│                     (User's Code)                           │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       │ imports
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                      src/index.ts                           │
│                   (Barrel Export)                           │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
        ▼              ▼              ▼
   ┌─────────┐   ┌──────────┐   ┌────────┐
   │ Client  │   │  Models  │   │ Errors │
   │ Module  │   │  Module  │   │ Module │
   └────┬────┘   └─────┬────┘   └────────┘
        │              │
        │              │
        ▼              ▼
   ┌────────┐    ┌────────┐
   │ Types  │    │ Utils  │
   │ Module │    │ Module │
   └────────┘    └────────┘
```

## Detailed Module View

### Client Module

```
client/
├── faber-client.ts
│   ├─ imports: ValidationError (from errors/)
│   ├─ imports: FaberConfig, TaskResult (from types/)
│   ├─ imports: ExecutionStep (from models/)
│   └─ exports: FaberClient class
│
└── index.ts
    └─ exports: FaberClient (barrel)
```

**Dependencies**: errors/, types/, models/

**Purpose**: HTTP communication with Faber API

**Key Methods**:

- `constructor(config)` - Initialize client
- `execute(steps)` - Execute task steps
- `normalizeResponse()` - Convert API response

### Models Module

```
models/
├── task.ts
│   ├─ imports: TestFunction (from types/)
│   ├─ exports: Task interface
│   └─ exports: ExecutionStep type
│
├── task-group.ts
│   ├─ imports: FaberClient (from client/)
│   ├─ imports: TaskResult, TestResult (from types/)
│   ├─ imports: zip (from utils/)
│   ├─ imports: Task, ExecutionStep (from ./task)
│   └─ exports: TaskGroup class
│
└── index.ts
    ├─ exports: TaskGroup
    └─ exports: Task, ExecutionStep (types)
```

**Dependencies**: client/, types/, utils/

**Purpose**: Domain logic and task orchestration

**Key Classes**:

- `TaskGroup` - Manages task execution sequences
  - `single()` - Add sequential task
  - `parallel()` - Add parallel tasks
  - `execute()` - Run all tasks

### Types Module

```
types/
├── config.ts
│   ├─ exports: FaberConfig interface
│   └─ exports: HealthResponse interface
│
├── execution.ts
│   ├─ exports: ExecutionStats interface
│   ├─ exports: TaskResult interface
│   ├─ exports: ExecutionResult interface
│   └─ exports: TaskGroupResult type
│
├── test.ts
│   ├─ imports: TaskResult (from ./execution)
│   ├─ exports: TestResult interface
│   └─ exports: TestFunction type
│
└── index.ts
    └─ exports: All types (barrel)
```

**Dependencies**: None (pure types)

**Purpose**: Type definitions for the entire SDK

**Key Types**:

- Configuration: `FaberConfig`
- Execution: `TaskResult`, `ExecutionResult`
- Testing: `TestResult`, `TestFunction`

### Errors Module

```
errors/
├── faber-error.ts
│   └─ exports: FaberError (base class)
│
├── connection-error.ts
│   ├─ imports: FaberError
│   └─ exports: ConnectionError
│
├── timeout-error.ts
│   ├─ imports: FaberError
│   └─ exports: TimeoutError
│
├── validation-error.ts
│   ├─ imports: FaberError
│   └─ exports: ValidationError
│
├── execution-error.ts
│   ├─ imports: FaberError
│   └─ exports: ExecutionError
│
├── api-error.ts
│   ├─ imports: FaberError
│   └─ exports: ApiError
│
└── index.ts
    └─ exports: All errors (barrel)
```

**Dependencies**: None (except internal)

**Purpose**: Custom error classes

**Error Hierarchy**:

```
Error (built-in)
  └── FaberError
       ├── ConnectionError
       ├── TimeoutError
       ├── ValidationError
       ├── ExecutionError
       └── ApiError
```

### Utils Module

```
utils/
├── array.ts
│   └─ exports: zip function
│
└── index.ts
    └─ exports: All utilities (barrel)
```

**Dependencies**: None

**Purpose**: Reusable utility functions

**Key Functions**:

- `zip(a, b)` - Combine two arrays into tuples

## Data Flow

### Task Execution Flow

```
User Code
   │
   ├─> Creates FaberClient
   │     └─> Validates config (throws ValidationError)
   │
   ├─> Creates TaskGroup(client)
   │
   ├─> Adds tasks (.single(), .parallel())
   │
   └─> Calls taskGroup.execute()
         │
         ├─> TaskGroup calls client.execute(steps)
         │     │
         │     ├─> Client makes HTTP request
         │     │
         │     ├─> Receives API response (snake_case)
         │     │
         │     └─> Normalizes to TaskResult (camelCase)
         │
         ├─> TaskGroup processes results
         │     │
         │     ├─> Zips results with tasks
         │     │
         │     ├─> Runs test functions (if provided)
         │     │
         │     └─> Creates TestResult objects
         │
         └─> Returns TestResult[]
               │
               └─> User receives results
```

## Import Patterns

### Recommended (Public API)

```typescript
// ✅ Import from main package
import { FaberClient, TaskGroup, ValidationError } from '@faber/sdk';
import type { Task, FaberConfig } from '@faber/sdk';
```

### Advanced (Direct Module Access)

```typescript
// ⚠️ Direct module imports (not recommended but supported)
import { FaberClient } from '@faber/sdk/client';
import { TaskGroup } from '@faber/sdk/models';
import type { Task } from '@faber/sdk/models';
```

### Internal (SDK Development Only)

```typescript
// 🔒 Internal imports (for SDK development)
import { FaberClient } from './client/faber-client';
import { TaskGroup } from './models/task-group';
import { zip } from './utils/array';
```

## Module Characteristics

| Module  | Dependencies          | Exports          | Side Effects | Testability |
| ------- | --------------------- | ---------------- | ------------ | ----------- |
| types/  | None                  | Type definitions | No           | N/A         |
| errors/ | None                  | Error classes    | No           | High        |
| utils/  | None                  | Pure functions   | No           | High        |
| client/ | errors, types, models | FaberClient      | Yes (HTTP)   | Medium      |
| models/ | client, types, utils  | TaskGroup, Task  | No           | High        |

## Circular Dependency Prevention

The structure prevents circular dependencies through:

1. **Layered Architecture**:

   - Base layer: types/, errors/, utils/ (no dependencies)
   - Middle layer: client/, models/ (depend on base)
   - Top layer: index.ts (depends on all)

2. **Dependency Direction**:

   - Always flows downward (never upward)
   - No sibling dependencies that create cycles
   - client/ and models/ reference each other carefully

3. **Type-Only Imports**:
   - Using `import type` where possible
   - Prevents runtime circular dependencies

## Testing Strategy

### Unit Tests

```
tests/
├── client/
│   └── faber-client.test.ts (mock fetch)
├── models/
│   └── task-group.test.ts (mock client)
├── errors/
│   └── errors.test.ts
└── utils/
    └── array.test.ts
```

### Integration Tests

```
tests/integration/
├── e2e.test.ts (full flow)
└── api.test.ts (real API calls)
```

## Future Extensions

The structure easily supports:

1. **New Clients**: Add to `client/` (e.g., `websocket-client.ts`)
2. **New Models**: Add to `models/` (e.g., `workflow.ts`)
3. **New Types**: Add to `types/` (e.g., `streaming.ts`)
4. **New Errors**: Add to `errors/` (e.g., `rate-limit-error.ts`)
5. **New Utils**: Add to `utils/` (e.g., `string.ts`, `validation.ts`)

---

**Legend**:

- `─>` : exports or provides to
- `├─` : has or contains
- `└─` : last item in group
- `│` : continuation
- `▼` : flows down to

