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
  ExecuteOptions,
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
} from './types';
export type { FaberClientConstructor } from './client';

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

// Type guards
export {
  isCompletedTaskResult,
  isFailedTaskResult,
  isSingleExecutionStep,
  isParallelExecutionStep,
  isSingleExecutionStepResult,
  isParallelExecutionStepResult,
} from './types';

// Version
export const SDK_VERSION = '0.1.0';

// Default configuration
export const DEFAULT_CONFIG = {
  baseUrl: 'http://localhost:3000',
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
} as const;