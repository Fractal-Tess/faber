/**
 * TaskGroup model for managing task execution sequences
 */

import type { FaberClient } from '../client';
import type { TaskResult, TestResult } from '../types';
import { zip } from '../utils';
import type { ExecutionStep, Task } from './task';

/**
 * TaskGroup allows you to build and manage a sequence of tasks,
 * supporting both sequential and parallel execution.
 *
 * @example
 * ```typescript
 * const taskGroup = new TaskGroup(client);
 *
 * taskGroup
 *   .single({ cmd: 'echo', args: ['Hello'] })
 *   .parallel([
 *     { cmd: 'echo', args: ['World'] },
 *     { cmd: 'echo', args: ['!'] }
 *   ]);
 *
 * const results = await taskGroup.execute();
 * ```
 */
export class TaskGroup {
  private executionSteps: ExecutionStep[] = [];

  /**
   * Creates a new TaskGroup.
   * @param client - The FaberClient instance to use for execution
   */
  constructor(private readonly client: FaberClient) {}

  /**
   * Adds a single task to the group to be executed sequentially.
   * @param task - The task to add
   * @returns The TaskGroup instance for method chaining
   */
  public single(task: Task): this {
    this.executionSteps.push(task);
    return this;
  }

  /**
   * Adds multiple tasks to be executed in parallel at this step.
   * @param tasks - The array of tasks to execute in parallel
   * @returns The TaskGroup instance for method chaining
   */
  public parallel(tasks: Task[]): this {
    this.executionSteps.push(tasks);
    return this;
  }

  /**
   * Executes all tasks in the group and returns test results.
   * @returns Promise resolving to test results for each step
   */
  public async execute(): Promise<(TestResult | TestResult[])[]> {
    const data = await this.client.execute(this.executionSteps);

    return zip(data, this.executionSteps).map(([step, executionStep]) => {
      if (Array.isArray(executionStep)) {
        return this.executeParallelStep(executionStep, step as TaskResult[]);
      }
      return this.executeSingleStep(executionStep, step as TaskResult);
    });
  }

  /**
   * Executes test for a single task step
   */
  private executeSingleStep(task: Task, result: TaskResult): TestResult {
    if (task.test) {
      return task.test(result);
    }

    return {
      passing: result.exitCode === 0,
      message: 'No test provided',
      ctx: result,
    };
  }

  /**
   * Executes tests for parallel task steps
   */
  private executeParallelStep(
    tasks: Task[],
    results: TaskResult[]
  ): TestResult[] {
    return zip(tasks, results).map(([task, result]) => {
      if (task.test) {
        return task.test(result);
      }

      return {
        passing: result.exitCode === 0,
        message: 'No test provided',
        ctx: result,
      };
    });
  }
}

