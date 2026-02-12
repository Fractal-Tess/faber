/**
 * Execution-related types for the Faber SDK
 */

/**
 * Execution statistics from task execution
 */
export interface ExecutionStats {
  memory_peak_bytes: number;
  cpu_usage_percent: number;
  pids_peak: number;
  execution_time_ms: number;
}

/**
 * Task execution result
 */
export interface TaskResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  stats?: ExecutionStats;
}

/**
 * Raw execution result from the API
 */
export interface ExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  stats?: ExecutionStats;
}

/**
 * Final result type that maintains single/parallel structure
 */
export type TaskGroupResult = (ExecutionResult | ExecutionResult[])[];

