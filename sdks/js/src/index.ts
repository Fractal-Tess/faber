/**
 * Faber Runtime SDK - JavaScript/TypeScript Client
 *
 * A secure, sandboxed task execution runtime SDK
 */

// Core exports
export { FaberClient } from './client';
export { TaskBuilder } from './task-builder';

export type {
  FaberClientConfig,
  Task,
  TaskGroup,
  TaskResult,
  TaskStats,
  TaskResultStats,
  CompletedTaskResult,
  FailedTaskResult,
  ExecutionStep,
  ExecutionStepResult,
  TaskGroupResult,
  HealthResponse,
  // Task testing types
  TestContext,
  TestResult,
  TaskWithTest,
  TaskGroupWithTests,
  TaskGroupResultWithTests,
} from './types';

// Error exports
export {
  FaberError,
  ConnectionError,
  TimeoutError,
  ValidationError,
  ExecutionError,
  ApiError,
} from './errors';

// Utility exports
export {
  getSuccessfulResults,
  getFailedResults,
  allTasksSucceeded,
  anyTaskSucceeded,
  getTotalExecutionTime,
  getTotalMemoryUsage,
  formatExecutionTime,
  formatMemorySize,
  createCommandTask,
  createCompilationTask,
  createScriptTask,
  ensureAllTasksSucceeded,
} from './utils';

// Default configuration
export const DEFAULT_CONFIG = {
  baseUrl: 'http://localhost:3000',
} as const;
