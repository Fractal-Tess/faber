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

// Builders
export { TaskBuilder } from './builders';
export type { TaskWithTests } from './builders';

// Models
export { TaskGroup } from './models';
export type { Task, ExecutionStep, SandboxProfile } from './models';

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

// Test Types
export type {
  TaskTest,
  EqualsTest,
  ContainsTest,
  MatchesTest,
  CustomTest,
  TaskTestResult,
  StepWithTestsResult,
  SingleStepWithTestsResult,
  ParallelStepWithTestsResult,
  ExecutionWithTestsResult,
} from './types/tests';

// Utils
export { TestResultAnalyzer, runTests, stripTests, stripTestsFromStep } from './utils';
export type { FailedStepInfo, StageSummary, ReportOptions } from './utils';

// Errors
export {
  FaberError,
  ConnectionError,
  TimeoutError,
  ValidationError,
  ExecutionError,
  ApiError,
} from './errors';
