/**
 * Execution-related types for the Faber SDK
 */

/**
 * Execution statistics from task execution
 */
export type TaskOutcome =
  | 'exited'
  | 'signaled'
  | 'timed_out'
  | 'out_of_memory'
  | 'pids_limit'
  | 'output_limit'
  | 'policy_violation'
  | 'infrastructure_failure';

export type ExecutionStats = {
  memory_peak_bytes: number;
  cpu_usage_usec: number;
  cpu_nr_throttled: number;
  cpu_throttled_usec: number;
  pids_peak: number;
  execution_time_ms: number;
  stdout_truncated: boolean;
  stderr_truncated: boolean;
  outcome: TaskOutcome;
  termination_signal: number | null;
  oom_kill_count: number;
  pids_limit_hit_count: number;
  cleanup_succeeded: boolean;
};

/**
 * Task execution result
 */
export type CompletedTaskResult = {
  stdout: string;
  stderr: string;
  exitCode: number;
  error?: never;
  stats?: ExecutionStats;
};

export type FailedTaskResult = {
  error: string;
  stdout?: never;
  stderr?: never;
  exitCode?: never;
  stats?: ExecutionStats;
};

export type TaskResult = CompletedTaskResult | FailedTaskResult;

/**
 * Raw execution result from the API
 */
export type ExecutionResult = TaskResult;

/**
 * Final result type that maintains single/parallel structure
 */
export type TaskGroupResult = (ExecutionResult | ExecutionResult[])[];
