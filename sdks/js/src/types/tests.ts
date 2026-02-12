/**
 * Test types for the Faber SDK client-side testing framework
 * 
 * These types support defining tests on tasks that are stripped before
 * sending to the API but executed locally to validate task results.
 */

import type { Task } from '../models/task';
import type { TaskResult } from './execution';

/**
 * Union type for all test types
 */
export type TaskTest =
  | EqualsTest
  | ContainsTest
  | MatchesTest
  | CustomTest;

/**
 * Test that checks if a field equals an expected value
 */
export type EqualsTest = {
  name: string;
  assertion: 'equals';
  field: 'stdout' | 'stderr' | 'exitCode';
  expected: string | number;
};

/**
 * Test that checks if a field contains a substring
 */
export type ContainsTest = {
  name: string;
  assertion: 'contains';
  field: 'stdout' | 'stderr';
  expected: string;
};

/**
 * Test that checks if a field matches a regular expression
 */
export type MatchesTest = {
  name: string;
  assertion: 'matches';
  field: 'stdout' | 'stderr';
  expected: RegExp;
};

/**
 * Test that uses a custom function for validation
 */
export type CustomTest = {
  name: string;
  assertion: 'custom';
  testFn: (result: TaskResult) => TaskTestResult;
};

/**
 * Result of running a single test
 */
export type TaskTestResult = {
  name: string;
  passed: boolean;
  message: string;
  expected?: unknown;
  actual?: unknown;
};

/**
 * Task with optional tests attached
 */
export type TaskWithTests = Task & {
  tests?: TaskTest[];
};

/**
 * Union type for step results with tests
 */
export type StepWithTestsResult =
  | SingleStepWithTestsResult
  | ParallelStepWithTestsResult;

/**
 * Result for a single task with tests
 */
export type SingleStepWithTestsResult = {
  stepIndex: number;
  parallel: false;
  task: TaskWithTests;
  result: TaskResult;
  testResults: TaskTestResult[];
  passed: boolean;
};

/**
 * Result for parallel tasks with tests
 */
export type ParallelStepWithTestsResult = {
  stepIndex: number;
  parallel: true;
  results: Array<{
    task: TaskWithTests;
    result: TaskResult;
    testResults: TaskTestResult[];
    testsPassed: boolean;
  }>;
  passed: boolean;
};

/**
 * Complete execution result with all test results
 */
export type ExecutionWithTestsResult = {
  results: TaskGroupResult;
  stepResults: StepWithTestsResult[];
  allTestsPassed: boolean;
  passedCount: number;
  failedCount: number;
};

import type { TaskGroupResult } from './execution';
