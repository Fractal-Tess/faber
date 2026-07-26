/**
 * Task model and related types
 */

import type { TestFunction } from '../types';

export type SandboxProfile = 'compile_v1' | 'native_v1';

/**
 * Represents a single executable task configuration.
 */
export type Task = {
  /** Command to execute */
  cmd: string;
  /** Command arguments */
  args?: string[];
  /** Environment variables */
  env?: Record<string, string>;
  /** Standard input to provide to the command */
  stdin?: string;
  /** Files to create in the task's working directory */
  files?: Record<string, string>;
  /** Working directory for command execution */
  working_dir?: string;
  /** Versioned seccomp workload policy (defaults to compile_v1) */
  sandbox_profile?: SandboxProfile;
  /** Optional test function to validate the task's result */
  test?: TestFunction;
};

/**
 * ExecutionStep is a single task or a list of tasks to be executed in parallel.
 */
export type ExecutionStep = Task | Task[];

