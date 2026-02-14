/**
 * API response types (snake_case format from server)
 */

import type { ExecutionStats } from './execution';

/**
 * Raw task result from the API (uses snake_case)
 */
export interface ApiTaskResult {
  stdout: string;
  stderr: string;
  exit_code: number;
  stats?: ExecutionStats;
}

/**
 * API response type for task execution
 */
export type ApiExecutionResponse = (ApiTaskResult | ApiTaskResult[])[];

