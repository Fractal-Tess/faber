/**
 * Execution-related types for the Faber SDK
 */

/**
 * Execution statistics from task execution
 */
export type ExecutionStats = {
  memory_peak_bytes: number;
  cpu_usage_usec: number;
  pids_peak: number;
  execution_time_ms: number;
  stdout_truncated: boolean;
  stderr_truncated: boolean;
};

/**
 * Task execution result
 */
export type TaskResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  stats?: ExecutionStats;
};

/**
 * Raw execution result from the API
 */
export type ExecutionResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  stats?: ExecutionStats;
};

/**
 * Final result type that maintains single/parallel structure
 */
export type TaskGroupResult = (ExecutionResult | ExecutionResult[])[];

