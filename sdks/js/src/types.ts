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

export type HealthResponse = {
  status: string;
  timestamp?: string;
};

// Task testing types
export type TestContext = {
  stdout: string;
  stderr: string;
  exit_code: number;
  stats: TaskResultStats;
  task: Task;
};

export type TestResult = {
  passed: boolean;
  message: string;
  details?: Record<string, any>;
};

export type TaskWithTest = Task & {
  test?: (context: TestContext) => TestResult;
};

export type TaskGroupWithTests = (Task | TaskWithTest)[];

// Combined result for a single step (execution + test)
export type TaskStepResult = {
  executionResult: ExecutionStepResult;
  testResult?: TestResult | TestResult[];
  passed: boolean;
};

export type StructuredTaskStepResults = TaskStepResult[];

export type TaskGroupResultWithTests = {
  results: TaskGroupResult;
  stepResults: StructuredTaskStepResults;
  allTestsPassed: boolean;
  failedSteps: TaskStepResult[];
};
