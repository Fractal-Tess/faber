/**
 * API response types (snake_case format from server)
 */

import type { ExecutionStats } from './execution';

/**
 * Raw task result from the API (uses snake_case)
 */
export type ApiCompletedTaskResult = {
  stdout: string;
  stderr: string;
  exit_code: number;
  error?: never;
  stats?: ExecutionStats;
};

export type ApiFailedTaskResult = {
  error: string;
  stdout?: never;
  stderr?: never;
  exit_code?: never;
  stats?: ExecutionStats;
};

export type ApiTaskResult = ApiCompletedTaskResult | ApiFailedTaskResult;

/**
 * API response type for task execution
 */
export type ApiExecutionResponse = (ApiTaskResult | ApiTaskResult[])[];
