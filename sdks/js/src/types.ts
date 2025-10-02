/**
 * Core types for the Faber runtime SDK
 */

export type Task = {
  cmd: string;
  args?: string[];
  env?: Record<string, string>;
  stdin?: string;
  files?: Record<string, string>;
  working_dir?: string;
};

export type TaskStats = {
  cpu_usage_usec: number;
  memory_peak_bytes: number;
  pids_max: number;
};

export type TaskResultStats = {
  memory_peak_bytes: number;
  cpu_usage_percent: number;
  pids_peak: number;
  execution_time_ms: number;
};

export type CompletedTaskResult = {
  stdout: string;
  stderr: string;
  exit_code: number;
  stats: TaskResultStats;
};

export type FailedTaskResult = {
  error: string;
  stats: TaskResultStats;
};

export type TaskResult = CompletedTaskResult | FailedTaskResult;

export type ExecutionStep = Task | Task[];
export type ExecutionStepResult = TaskResult | TaskResult[];
export type TaskGroup = ExecutionStep[];
export type TaskGroupResult = ExecutionStepResult[];

export type FaberClientConfig = {
  baseUrl: string;
  timeout?: number;
  headers?: Record<string, string>;
  apiKey?: string;
};

export type ExecuteOptions = {
  timeout?: number;
  headers?: Record<string, string>;
};

export type HealthResponse = {
  status: string;
  timestamp?: string;
};

/**
 * Type guards
 */
export function isCompletedTaskResult(result: TaskResult): result is CompletedTaskResult {
  return 'stdout' in result && 'stderr' in result && 'exit_code' in result;
}

export function isFailedTaskResult(result: TaskResult): result is FailedTaskResult {
  return 'error' in result;
}

export function isSingleExecutionStep(step: ExecutionStep): step is Task {
  return !Array.isArray(step);
}

export function isParallelExecutionStep(step: ExecutionStep): step is Task[] {
  return Array.isArray(step);
}

export function isSingleExecutionStepResult(result: ExecutionStepResult): result is TaskResult {
  return !Array.isArray(result);
}

export function isParallelExecutionStepResult(result: ExecutionStepResult): result is TaskResult[] {
  return Array.isArray(result);
}