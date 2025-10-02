import {
  FaberClientConfig,
  ExecuteOptions,
  TaskGroup,
  TaskGroupResult,
  HealthResponse,
  Task,
} from './types';
import { TaskBuilder } from './task-builder';
import {
  FaberError,
  ConnectionError,
  TimeoutError,
  ValidationError,
  ApiError,
} from './errors';

/**
 * Faber Runtime SDK Client
 *
 * Provides a JavaScript/TypeScript interface to the Faber secure task execution runtime API.
 */
export type FaberClientConstructor = {
  baseUrl: string;
  timeout?: number;
  headers?: Record<string, string>;
  apiKey?: string;
};

export class FaberClient {
  private baseUrl: string;
  private defaultTimeout: number;
  private defaultHeaders: Record<string, string>;
  private apiKey?: string;

  constructor(config: FaberClientConstructor) {
    if (!config.baseUrl) {
      throw new ValidationError('baseUrl is required');
    }

    // Ensure baseUrl doesn't end with a slash
    this.baseUrl = config.baseUrl.replace(/\/$/, '');
    this.defaultTimeout = config.timeout || 30000; // 30 seconds default
    this.apiKey = config.apiKey;
    this.defaultHeaders = {
      'Content-Type': 'application/json',
      ...config.headers,
    };
  }

  /**
   * Get headers with API key authentication
   */
  private getAuthHeaders(optionsHeaders?: Record<string, string>): Record<string, string> {
    const headers = { ...this.defaultHeaders, ...optionsHeaders };

    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    return headers;
  }

  /**
   * Execute a task group on the Faber runtime
   */
  async executeGroup(tasks: TaskGroup | TaskBuilder, options?: ExecuteOptions): Promise<TaskGroupResult> {
    // Handle TaskBuilder
    const actualTasks = tasks instanceof TaskBuilder ? tasks.build() : tasks;

    if (!actualTasks || actualTasks.length === 0) {
      throw new ValidationError('Task group cannot be empty');
    }

    // Validate task structure
    this.validateTaskGroup(actualTasks);

    const url = `${this.baseUrl}/api/v1/execute`;
    const timeout = options?.timeout || this.defaultTimeout;
    const headers = this.getAuthHeaders(options?.headers);

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeout);

      const response = await fetch(url, {
        method: 'POST',
        headers,
        body: JSON.stringify(actualTasks),
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

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
        if (error.name === 'AbortError') {
          throw new TimeoutError();
        }
        throw new ConnectionError(`Failed to connect to Faber runtime: ${error.message}`, error);
      }

      throw new FaberError(`Unexpected error: ${error}`);
    }
  }

  /**
   * Check the health of the Faber runtime
   */
  async health(options?: ExecuteOptions): Promise<HealthResponse> {
    const url = `${this.baseUrl}/api/v1/health`;
    const timeout = options?.timeout || this.defaultTimeout;
    const headers = this.getAuthHeaders(options?.headers);

    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeout);

      const response = await fetch(url, {
        method: 'GET',
        headers,
        signal: controller.signal,
      });

      clearTimeout(timeoutId);

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
        if (error.name === 'AbortError') {
          throw new TimeoutError();
        }
        throw new ConnectionError(`Failed to connect to Faber runtime: ${error.message}`, error);
      }

      throw new FaberError(`Unexpected error: ${error}`);
    }
  }

  /**
   * Execute a single task
   */
  async executeSingle(task: Task, options?: ExecuteOptions): Promise<TaskGroupResult> {
    if (!task.cmd || typeof task.cmd !== 'string') {
      throw new ValidationError('Task must have a valid cmd property');
    }

    return this.executeGroup([task], options);
  }

  /**
   * Execute multiple tasks in parallel
   */
  async executeParallel(tasks: Task[], options?: ExecuteOptions): Promise<TaskGroupResult> {
    if (!Array.isArray(tasks) || tasks.length === 0) {
      throw new ValidationError('Tasks must be a non-empty array');
    }

    // Validate each task
    for (let i = 0; i < tasks.length; i++) {
      if (!tasks[i].cmd || typeof tasks[i].cmd !== 'string') {
        throw new ValidationError(`Task ${i} must have a valid cmd property`);
      }
    }

    return this.executeGroup([tasks], options);
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
      return await response.json();
    } catch {
      return { message: await response.text() };
    }
  }

  /**
   * Create a new client with different configuration
   */
  withConfig(config: Partial<FaberClientConstructor>): FaberClient {
    return new FaberClient({
      baseUrl: config.baseUrl || this.baseUrl,
      timeout: config.timeout || this.defaultTimeout,
      headers: { ...this.defaultHeaders, ...config.headers },
      apiKey: config.apiKey || this.apiKey,
    });
  }
}