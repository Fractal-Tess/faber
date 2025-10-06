import {
  TaskGroup,
  TaskGroupResult,
  HealthResponse,
  Task,
  TaskWithTest,
  TaskGroupWithTests,
  TaskGroupResultWithTests,
  TestContext,
  TestResult,
  CompletedTaskResult,
} from './types';
import { TaskBuilder } from './task-builder';
import {
  FaberError,
  ConnectionError,
  ValidationError,
  ApiError,
} from './errors';
import { FaberConfig, faberConfigSchema } from './schemas';

/**
 * Faber Runtime SDK Client
 *
 * Provides a JavaScript/TypeScript interface to the Faber secure task execution runtime API.
 */
export class FaberClient {
  private baseUrl: string;
  private apiKey: string;
  private fetchFn: typeof fetch;

  constructor(config: FaberConfig) {
    const { success, data, error } = faberConfigSchema.safeParse(config);
    if (!success) {
      throw new ValidationError('Invalid configuration', error.message);
    }

    // Ensure baseUrl doesn't end with a slash
    this.baseUrl = data.baseUrl;
    this.apiKey = data.apiKey;
    this.fetchFn = (data.fetch as typeof fetch) ?? fetch;
  }

  /**
   * Get the request headers
   */
  private getHeaders(): Record<string, string> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    return headers;
  }

  /**
   * Execute a task group on the Faber runtime
   */
  async executeGroup(tasks: TaskGroup | TaskBuilder): Promise<TaskGroupResult> {
    // Handle TaskBuilder
    const actualTasks = tasks instanceof TaskBuilder ? tasks.build() : tasks;

    if (!actualTasks || actualTasks.length === 0) {
      throw new ValidationError('Task group cannot be empty');
    }

    // Validate task structure
    this.validateTaskGroup(actualTasks);

    const url = `${this.baseUrl}/api/v1/execute`;
    const headers = this.getHeaders();

    try {
      const response = await this.fetchFn(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(actualTasks),
      });

      if (!response.ok) {
        const errorData = await this.tryParseError(response);
        throw new ApiError(
          `HTTP ${response.status}: ${response.statusText}`,
          response.status,
          response.statusText,
          errorData
        );
      }

      const result = await response.json();
      return result as TaskGroupResult;
    } catch (error) {
      if (error instanceof FaberError) {
        throw error;
      }

      if (error instanceof Error) {
        throw new ConnectionError(
          `Failed to connect to Faber runtime: ${error.message}`,
          error
        );
      }

      throw new FaberError(`Unexpected error: ${error}`);
    }
  }

  /**
   * Execute a task group with tests on the Faber runtime
   */
  async executeGroupWithTests(
    tasks: TaskGroupWithTests | TaskBuilder
  ): Promise<TaskGroupResultWithTests> {
    // Handle TaskBuilder - convert to TaskWithTest array
    let actualTasks: TaskGroupWithTests;
    if (tasks instanceof TaskBuilder) {
      actualTasks = tasks.build() as TaskGroupWithTests;
    } else {
      actualTasks = tasks;
    }

    if (!actualTasks || actualTasks.length === 0) {
      throw new ValidationError('Task group cannot be empty');
    }

    // Extract regular tasks for execution (strip test functions)
    const executionTasks: TaskGroup = actualTasks.map((task) => {
      if (Array.isArray(task)) {
        // Parallel tasks - strip test from each task
        return task.map((t) => {
          const { test, ...taskWithoutTest } = t as TaskWithTest;
          return taskWithoutTest;
        });
      } else {
        // Single task - strip test
        const { test, ...taskWithoutTest } = task as TaskWithTest;
        return taskWithoutTest;
      }
    });

    // Validate task structure
    this.validateTaskGroup(executionTasks);

    const url = `${this.baseUrl}/api/v1/execute`;
    const headers = this.getHeaders();

    try {
      const response = await this.fetchFn(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(executionTasks),
      });

      if (!response.ok) {
        const errorData = await this.tryParseError(response);
        throw new ApiError(
          `HTTP ${response.status}: ${response.statusText}`,
          response.status,
          response.statusText,
          errorData
        );
      }

      const results = (await response.json()) as TaskGroupResult;
      const stepResults = this.createStepResults(actualTasks, results);

      return {
        results,
        stepResults,
        allTestsPassed: stepResults.every((step) => step.passed),
        failedSteps: stepResults.filter((step) => !step.passed),
      };
    } catch (error) {
      if (error instanceof FaberError) {
        throw error;
      }

      if (error instanceof Error) {
        throw new ConnectionError(
          `Failed to connect to Faber runtime: ${error.message}`,
          error
        );
      }

      throw new FaberError(`Unexpected error: ${error}`);
    }
  }

  /**
   * Create step results that combine execution and test results
   */
  private createStepResults(
    tasksWithTests: TaskGroupWithTests,
    results: TaskGroupResult
  ): StructuredTaskStepResults {
    const stepResults: StructuredTaskStepResults = [];

    for (let i = 0; i < tasksWithTests.length; i++) {
      const taskWithTest = tasksWithTests[i];
      const executionResult = results[i];

      // Handle both single tasks and parallel task arrays
      if (Array.isArray(taskWithTest)) {
        // Parallel tasks - create step result with array of execution results and array of test results
        const parallelExecutionResults = executionResult as TaskResult[];
        const parallelTestResults: (TestResult | undefined)[] = [];
        let allPassed = true;

        taskWithTest.forEach((task, taskIndex) => {
          const taskExecutionResult = parallelExecutionResults[taskIndex];
          let testResult: TestResult | undefined;

          if (
            'test' in task &&
            task.test &&
            this.isCompletedTaskResult(taskExecutionResult)
          ) {
            try {
              const testContext: TestContext = {
                stdout: taskExecutionResult.stdout,
                stderr: taskExecutionResult.stderr,
                exit_code: taskExecutionResult.exit_code,
                stats: taskExecutionResult.stats,
                task: task,
              };
              testResult = task.test(testContext);
              if (!testResult.passed) {
                allPassed = false;
              }
            } catch (error) {
              testResult = {
                passed: false,
                message: `Test execution failed: ${
                  error instanceof Error ? error.message : 'Unknown error'
                }`,
                details: {
                  error: error instanceof Error ? error.stack : error,
                },
              };
              allPassed = false;
            }
          }

          parallelTestResults.push(testResult);
        });

        stepResults.push({
          executionResult: parallelExecutionResults,
          testResult: parallelTestResults,
          passed: allPassed,
        });
      } else {
        // Single task
        let testResult: TestResult | undefined;
        let passed = true;

        if (
          'test' in taskWithTest &&
          taskWithTest.test &&
          this.isCompletedTaskResult(executionResult)
        ) {
          try {
            const testContext: TestContext = {
              stdout: executionResult.stdout,
              stderr: executionResult.stderr,
              exit_code: executionResult.exit_code,
              stats: executionResult.stats,
              task: taskWithTest,
            };
            testResult = taskWithTest.test(testContext);
            passed = testResult.passed;
          } catch (error) {
            testResult = {
              passed: false,
              message: `Test execution failed: ${
                error instanceof Error ? error.message : 'Unknown error'
              }`,
              details: { error: error instanceof Error ? error.stack : error },
            };
            passed = false;
          }
        }

        stepResults.push({
          executionResult,
          testResult,
          passed,
        });
      }
    }

    return stepResults;
  }

  /**
   * Check if a task result is a completed task result (has stdout/stderr)
   */
  private isCompletedTaskResult(
    result: TaskResult
  ): result is CompletedTaskResult {
    return 'stdout' in result && 'stderr' in result;
  }

  /**
   * Check the health of the Faber runtime
   */
  async health(): Promise<HealthResponse> {
    const url = `${this.baseUrl}/api/v1/health`;
    const headers = this.getHeaders();

    try {
      const response = await this.fetchFn(url, {
        method: 'GET',
        headers,
      });

      if (!response.ok) {
        const errorData = await this.tryParseError(response);
        throw new ApiError(
          `HTTP ${response.status}: ${response.statusText}`,
          response.status,
          response.statusText,
          errorData
        );
      }

      const result = await response.json();
      return result as HealthResponse;
    } catch (error) {
      if (error instanceof FaberError) {
        throw error;
      }

      if (error instanceof Error) {
        throw new ConnectionError(
          `Failed to connect to Faber runtime: ${error.message}`,
          error
        );
      }

      throw new FaberError(`Unexpected error: ${error}`);
    }
  }

  /**
   * Execute a single task
   */
  async executeSingle(task: Task): Promise<TaskGroupResult> {
    if (!task.cmd || typeof task.cmd !== 'string') {
      throw new ValidationError('Task must have a valid cmd property');
    }

    return this.executeGroup([task]);
  }

  /**
   * Execute multiple tasks in parallel
   */
  async executeParallel(tasks: Task[]): Promise<TaskGroupResult> {
    if (!Array.isArray(tasks) || tasks.length === 0) {
      throw new ValidationError('Tasks must be a non-empty array');
    }

    // Validate each task
    for (let i = 0; i < tasks.length; i++) {
      if (!tasks[i].cmd || typeof tasks[i].cmd !== 'string') {
        throw new ValidationError(`Task ${i} must have a valid cmd property`);
      }
    }

    return this.executeGroup([tasks]);
  }

  /**
   * Validate task group structure
   */
  private validateTaskGroup(tasks: TaskGroup): void {
    const errors: string[] = [];

    for (let i = 0; i < tasks.length; i++) {
      const step = tasks[i];

      if (Array.isArray(step)) {
        // Parallel execution step
        if (step.length === 0) {
          errors.push(`Step ${i}: Parallel execution step cannot be empty`);
        }

        step.forEach((task, taskIndex) => {
          const taskErrors = this.validateTask(task, `${i}[${taskIndex}]`);
          errors.push(...taskErrors);
        });
      } else {
        // Single execution step
        const taskErrors = this.validateTask(step, `${i}`);
        errors.push(...taskErrors);
      }
    }

    if (errors.length > 0) {
      throw new ValidationError('Task group validation failed', errors);
    }
  }

  /**
   * Validate individual task
   */
  private validateTask(task: Task, path: string): string[] {
    const errors: string[] = [];

    if (!task.cmd || typeof task.cmd !== 'string') {
      errors.push(`${path}: cmd is required and must be a string`);
    }

    if (task.args && !Array.isArray(task.args)) {
      errors.push(`${path}: args must be an array of strings`);
    }

    if (task.env && typeof task.env !== 'object') {
      errors.push(`${path}: env must be an object`);
    }

    if (task.files && typeof task.files !== 'object') {
      errors.push(`${path}: files must be an object`);
    }

    return errors;
  }

  /**
   * Try to parse error response
   */
  private async tryParseError(response: Response): Promise<any> {
    try {
      const text = await response.text();
      try {
        return JSON.parse(text);
      } catch {
        return { message: text };
      }
    } catch {
      return { message: 'Unknown error' };
    }
  }

  /**
   * Create a new client with different configuration
   */
  withConfig(config: Partial<FaberClientConfig>): FaberClient {
    return new FaberClient({
      baseUrl: config.baseUrl || this.baseUrl,
      apiKey: config.apiKey || this.apiKey,
      fetch: config.fetch || this.fetchFn,
    });
  }
}
