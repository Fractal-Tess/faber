/**
 * Faber Runtime SDK Client
 */

import { ValidationError } from '../errors';
import type { ExecutionStep } from '../models';
import type {
  FaberConfig,
  TaskResult,
  ApiExecutionResponse,
  ApiTaskResult,
} from '../types';

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
      throw new Error(
        `API request failed: ${response.status} ${response.statusText}`
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
}
