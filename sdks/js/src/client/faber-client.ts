/**
 * Faber Runtime SDK Client
 */

import { ValidationError, ApiError } from '../errors';
import { TaskBuilder } from '../builders';
import { stripTestsFromStep } from '../utils/task-utils';
import { runTests } from '../utils/test-runner';
import type { ExecutionStep } from '../models';
import type {
  FaberConfig,
  TaskResult,
  ApiExecutionResponse,
  ApiTaskResult,
  TaskGroupResult,
  HealthResponse,
} from '../types';
import type { 
  TaskWithTests, 
  ExecutionWithTestsResult,
  StepWithTestsResult,
  SingleStepWithTestsResult,
  ParallelStepWithTestsResult
} from '../types/tests';

/**
 * FaberClient provides a JavaScript/TypeScript interface to the Faber
 * secure task execution runtime API.
 *
 * @example
 * ```typescript
 * const client = new FaberClient({
 *   baseUrl: 'http://localhost:3000',
 *   apiKey: 'your-api-key'
 * });
 * ```
 */
export class FaberClient {
  private readonly baseUrl: string;
  private readonly apiKey: string;
  private readonly fetchFn: typeof fetch;

  /**
   * Create a new FaberClient
   * @param config - The configuration for the FaberClient
   * @throws `ValidationError` if baseUrl or apiKey is not provided
   */
  constructor(config: FaberConfig) {
    if (!config.baseUrl) {
      throw new ValidationError('baseUrl is required');
    }
    if (!config.apiKey) {
      throw new ValidationError('apiKey is required');
    }

    this.baseUrl = config.baseUrl.replace(/\/$/, ''); // Remove trailing slash
    this.apiKey = config.apiKey;
    this.fetchFn = config.fetch ?? fetch;
  }

  /**
   * Get the request headers for API calls
   */
  private getHeaders(): Record<string, string> {
    return {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${this.apiKey}`,
    };
  }

  /**
   * Executes a sequence of tasks (internal method used by TaskGroup)
   * @internal
   */
  public async execute(
    executionSteps: ExecutionStep[]
  ): Promise<(TaskResult | TaskResult[])[]> {
    const response = await this.fetchFn(`${this.baseUrl}/api/v1/execute`, {
      method: 'POST',
      headers: this.getHeaders(),
      body: JSON.stringify(executionSteps),
    });

    if (!response.ok) {
      let errorBody: unknown;
      try {
        errorBody = await response.json();
      } catch {
        errorBody = await response.text().catch(() => '');
      }
      throw new ApiError(
        `API request failed: ${response.status} ${response.statusText}`,
        response.status,
        response.statusText,
        errorBody
      );
    }

    const data = await response.json();
    return this.normalizeResponse(data);
  }

  /**
   * Normalize API response from snake_case to camelCase
   */
  private normalizeResponse(
    data: ApiExecutionResponse
  ): (TaskResult | TaskResult[])[] {
    return data.map((step) => {
      if (Array.isArray(step)) {
        return step.map((result) => this.normalizeTaskResult(result));
      }
      return this.normalizeTaskResult(step);
    });
  }

  /**
   * Convert a single API result to TaskResult
   */
  private normalizeTaskResult(result: ApiTaskResult): TaskResult {
    return {
      stdout: result.stdout,
      stderr: result.stderr,
      exitCode: result.exit_code,
      stats: result.stats,
    };
  }

  async health(): Promise<HealthResponse> {
    const response = await this.fetchFn(`${this.baseUrl}/api/v1/health`);
    if (!response.ok) {
      throw new Error(`Health check failed: ${response.status}`);
    }
    return response.json();
  }

  async executeSingle(task: ExecutionStep): Promise<TaskResult> {
    const results = await this.execute([task]);
    return results[0] as TaskResult;
  }

  async executeGroup(
    steps: ExecutionStep[] | TaskBuilder
  ): Promise<TaskGroupResult> {
    const executionSteps = steps instanceof TaskBuilder 
      ? steps.build() 
      : steps;
    return this.execute(executionSteps);
  }

  async executeWithTests(
    steps: ExecutionStep[] | TaskBuilder
  ): Promise<ExecutionWithTestsResult> {
    const executionSteps = steps instanceof TaskBuilder 
      ? steps.build() 
      : steps;
    
    const cleanSteps = executionSteps.map(stripTestsFromStep);
    const results = await this.execute(cleanSteps);
    
    const stepResults: StepWithTestsResult[] = [];
    let allTestsPassed = true;
    let passedCount = 0;
    let failedCount = 0;

    for (let i = 0; i < executionSteps.length; i++) {
      const step = executionSteps[i];
      const result = results[i];
      
      if (Array.isArray(step)) {
        const parallelResults = step.map((task, idx) => {
          const taskResult = (result as TaskResult[])[idx];
          const testResults = runTests(task as TaskWithTests, taskResult);
          const testsPassed = testResults.every(t => t.passed);
          return {
            task: task as TaskWithTests,
            result: taskResult,
            testResults,
            testsPassed,
          };
        });
        
        const stepPassed = parallelResults.every(r => r.testsPassed);
        stepResults.push({
          stepIndex: i,
          parallel: true,
          results: parallelResults,
          passed: stepPassed,
        } as ParallelStepWithTestsResult);

        if (stepPassed) passedCount++;
        else { failedCount++; allTestsPassed = false; }
      } else {
        const testResults = runTests(step as TaskWithTests, result as TaskResult);
        const testsPassed = testResults.every(t => t.passed);
        
        stepResults.push({
          stepIndex: i,
          parallel: false,
          task: step as TaskWithTests,
          result: result as TaskResult,
          testResults,
          passed: testsPassed,
        } as SingleStepWithTestsResult);

        if (testsPassed) passedCount++;
        else { failedCount++; allTestsPassed = false; }
      }
    }
    
    return { results, stepResults, allTestsPassed, passedCount, failedCount };
  }
}
