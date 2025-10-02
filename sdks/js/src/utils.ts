import { Task, TaskResult, TaskGroupResult } from './types';
import { isCompletedTaskResult, isFailedTaskResult } from './types';
import { ExecutionError } from './errors';

/**
 * Utility functions for working with the Faber SDK
 */

/**
 * Extract successful results from a task group result
 */
export function getSuccessfulResults(results: TaskGroupResult): TaskResult[] {
  const successful: TaskResult[] = [];

  for (const stepResult of results) {
    if (Array.isArray(stepResult)) {
      // Parallel execution step
      successful.push(...stepResult.filter(isCompletedTaskResult));
    } else {
      // Single execution step
      if (isCompletedTaskResult(stepResult)) {
        successful.push(stepResult);
      }
    }
  }

  return successful;
}

/**
 * Extract failed results from a task group result
 */
export function getFailedResults(results: TaskGroupResult): TaskResult[] {
  const failed: TaskResult[] = [];

  for (const stepResult of results) {
    if (Array.isArray(stepResult)) {
      // Parallel execution step
      failed.push(...stepResult.filter(isFailedTaskResult));
    } else {
      // Single execution step
      if (isFailedTaskResult(stepResult)) {
        failed.push(stepResult);
      }
    }
  }

  return failed;
}

/**
 * Check if all tasks in a task group succeeded
 */
export function allTasksSucceeded(results: TaskGroupResult): boolean {
  return getFailedResults(results).length === 0;
}

/**
 * Check if any task in a task group succeeded
 */
export function anyTaskSucceeded(results: TaskGroupResult): boolean {
  return getSuccessfulResults(results).length > 0;
}

/**
 * Get total execution time across all tasks
 */
export function getTotalExecutionTime(results: TaskGroupResult): number {
  let totalTime = 0;

  for (const stepResult of results) {
    if (Array.isArray(stepResult)) {
      // Parallel execution step - take the max (they run in parallel)
      const maxTime = Math.max(...stepResult.map(r => r.stats.execution_time_ms));
      totalTime += maxTime;
    } else {
      // Single execution step
      totalTime += stepResult.stats.execution_time_ms;
    }
  }

  return totalTime;
}

/**
 * Get total memory usage across all tasks
 */
export function getTotalMemoryUsage(results: TaskGroupResult): number {
  let totalMemory = 0;

  for (const stepResult of results) {
    if (Array.isArray(stepResult)) {
      // Parallel execution step - sum all tasks
      totalMemory += stepResult.reduce((sum, r) => sum + r.stats.memory_peak_bytes, 0);
    } else {
      // Single execution step
      totalMemory += stepResult.stats.memory_peak_bytes;
    }
  }

  return totalMemory;
}

/**
 * Format execution time for human readable output
 */
export function formatExecutionTime(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`;
  } else if (ms < 60000) {
    return `${(ms / 1000).toFixed(2)}s`;
  } else {
    const minutes = Math.floor(ms / 60000);
    const seconds = ((ms % 60000) / 1000).toFixed(2);
    return `${minutes}m ${seconds}s`;
  }
}

/**
 * Format memory size for human readable output
 */
export function formatMemorySize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(2)} ${units[unitIndex]}`;
}

/**
 * Create a simple command task
 */
export function createCommandTask(
  cmd: string,
  args?: string[],
  options?: {
    env?: Record<string, string>;
    stdin?: string;
    files?: Record<string, string>;
    working_dir?: string;
  }
): Task {
  return {
    cmd,
    args,
    ...options,
  };
}

/**
 * Create a file compilation task
 */
export function createCompilationTask(
  sourceFile: string,
  outputFile: string,
  compiler: string = 'gcc',
  compilerArgs?: string[]
): Task {
  return createCommandTask(compiler, [
    ...(compilerArgs || []),
    sourceFile,
    '-o',
    outputFile,
  ]);
}

/**
 * Create a script execution task
 */
export function createScriptTask(
  scriptContent: string,
  interpreter: string = 'bash',
  scriptName: string = 'script.sh'
): Task {
  return createCommandTask(interpreter, [scriptName], {
    files: {
      [scriptName]: scriptContent,
    },
  });
}

/**
 * Throw an error if any task failed
 */
export function ensureAllTasksSucceeded(results: TaskGroupResult): void {
  const failed = getFailedResults(results);

  if (failed.length > 0) {
    const errorMessages = failed.map((result, index) => {
      if (isFailedTaskResult(result)) {
        return `Task ${index + 1}: ${result.error}`;
      }
      return '';
    }).filter(Boolean);

    throw new ExecutionError(
      `${failed.length} task(s) failed:\n${errorMessages.join('\n')}`
    );
  }
}