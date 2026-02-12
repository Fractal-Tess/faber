# Migration Guide

This guide helps you migrate from the old SDK structure to the new organized structure.

## What Changed

### Directory Structure

The SDK has been reorganized into a cleaner, more maintainable structure:

**Before:**

```
src/
├── client.ts
├── taskgroup.ts
├── types.ts
├── errors.ts
├── utils.ts
└── index.ts
```

**After:**

```
src/
├── client/
├── models/
├── types/
├── errors/
├── utils/
└── index.ts
```

## Breaking Changes

### None for Public API Users

If you're importing from the main package, **no changes are required**:

```typescript
// This still works exactly the same
import { FaberClient, TaskGroup } from '@faber/sdk';
import type { Task, FaberConfig } from '@faber/sdk';
```

### Internal Imports (If Used)

If you were importing from internal paths (not recommended), update as follows:

**Before:**

```typescript
import { FaberClient } from '@faber/sdk/client';
import { TaskGroup } from '@faber/sdk/taskgroup';
import type { Task } from '@faber/sdk/taskgroup';
import { ValidationError } from '@faber/sdk/errors';
```

**After:**

```typescript
import { FaberClient } from '@faber/sdk/client';
import { TaskGroup } from '@faber/sdk/models';
import type { Task } from '@faber/sdk/models';
import { ValidationError } from '@faber/sdk/errors';
```

## Improvements

### 1. Better Organization

- Related functionality is grouped together
- Easier to find specific code
- Clear separation of concerns

### 2. Enhanced Type Safety

- Types are now interfaces where appropriate
- Better type inference
- Improved IDE autocomplete

### 3. Improved Documentation

- Better JSDoc comments
- Type-level documentation
- Architecture documentation added

### 4. Better Error Handling

- Each error class in its own file
- Proper prototype chain setup
- Better stack traces

### 5. More Testable

- Modular structure makes mocking easier
- Each module can be tested in isolation
- Better dependency injection support

## Recommended Practices

### 1. Always Import from Main Package

```typescript
// ✅ Good
import { FaberClient, TaskGroup, ValidationError } from '@faber/sdk';

// ❌ Avoid
import { FaberClient } from '@faber/sdk/client/faber-client';
```

### 2. Use Type Imports for Types

```typescript
// ✅ Good - uses type import
import type { Task, FaberConfig } from '@faber/sdk';

// ⚠️ Works but not optimal
import { Task, FaberConfig } from '@faber/sdk';
```

### 3. Leverage Type Safety

```typescript
// ✅ Good - fully typed
const task: Task = {
  cmd: 'echo',
  args: ['hello'],
  test: (result) => ({
    passing: result.exitCode === 0,
    message: result.exitCode === 0 ? 'Success' : 'Failed',
  }),
};

// ❌ Avoid - loses type safety
const task = {
  cmd: 'echo',
  args: ['hello'],
} as any;
```

### 4. Handle Errors Properly

```typescript
import { FaberClient, ValidationError, ApiError } from '@faber/sdk';

try {
  const client = new FaberClient(config);
  const results = await taskGroup.execute();
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Configuration error:', error.message);
  } else if (error instanceof ApiError) {
    console.error('API error:', error.status, error.message);
  } else {
    console.error('Unexpected error:', error);
  }
}
```

## Testing Your Migration

After migrating, ensure:

1. **All imports resolve correctly**

   ```bash
   npm run build
   ```

2. **Types are working**

   ```bash
   npm run type-check
   ```

3. **Tests pass**

   ```bash
   npm test
   ```

4. **No runtime errors**
   - Test your application end-to-end
   - Verify error handling works
   - Check all SDK features you use

## Support

If you encounter any issues during migration:

1. Check this migration guide
2. Review the [ARCHITECTURE.md](./ARCHITECTURE.md)
3. Check the examples in `/examples`
4. Open an issue on GitHub

## Rollback

If you need to rollback:

1. Revert to the previous version in package.json
2. Run `npm install`
3. No code changes needed in your application

