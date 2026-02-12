/**
 * Faber Runtime SDK - JavaScript/TypeScript Client
 *
 * A secure, sandboxed task execution runtime SDK for executing commands
 * and managing complex task workflows.
 *
 * @example
 * ```typescript
 * import { FaberClient, TaskGroup } from '@faber/sdk';
 *
 * const client = new FaberClient({
 *   baseUrl: 'http://localhost:3000',
 *   apiKey: 'your-api-key'
 * });
 *
 * const taskGroup = new TaskGroup(client);
 * taskGroup
 *   .single({ cmd: 'echo', args: ['Hello, World!'] })
 *   .parallel([
 *     { cmd: 'ls', args: ['-la'] },
 *     { cmd: 'pwd' }
 *   ]);
 *
 * const results = await taskGroup.execute();
 * ```
 *
 * @packageDocumentation
 */

// Core client
export { FaberClient } from './client';

// Models
export { TaskGroup } from './models';
export type { Task, ExecutionStep } from './models';

// Types
export type {
  FaberConfig,
  HealthResponse,
  ExecutionStats,
  TaskResult,
  ExecutionResult,
  TaskGroupResult,
  TestResult,
  TestFunction,
} from './types';

// Errors
export {
  FaberError,
  ConnectionError,
  TimeoutError,
  ValidationError,
  ExecutionError,
  ApiError,
} from './errors';
