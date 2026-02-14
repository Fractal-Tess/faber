/**
 * Test-related types for the Faber SDK
 */

import type { TaskResult } from './execution';

/**
 * Test result from task execution
 */
export interface TestResult {
  passing: boolean;
  message: string;
  ctx?: TaskResult;
}

/**
 * Test function type
 */
export type TestFunction = (context: TaskResult) => TestResult;

