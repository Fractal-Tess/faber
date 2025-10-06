import { Task, TaskGroup, ExecutionStep, TaskWithTest, TestContext, TestResult } from './types';
import { ValidationError } from './errors';

/**
 * TaskBuilder provides a fluent API for building task execution plans
 * that match the Faber runtime's sequential execution model with parallel steps.
 */
export class TaskBuilder {
  private steps: ExecutionStep[] = [];

  /**
   * Create a new TaskBuilder instance
   */
  constructor() {
    this.steps = [];
  }

  /**
   * Add a single task to the execution plan
   * @param task - The task to execute
   * @returns This TaskBuilder instance for method chaining
   */
  single(task: Task): TaskBuilder {
    if (!task.cmd || typeof task.cmd !== 'string') {
      throw new ValidationError('Task must have a valid cmd property');
    }

    this.steps.push(task);
    return this;
  }

  /**
   * Add a single task with a test to the execution plan
   * @param task - The task to execute
   * @param test - The test function to validate the task result
   * @returns This TaskBuilder instance for method chaining
   */
  singleWithTest(task: Task, test: (context: TestContext) => TestResult): TaskBuilder {
    if (!task.cmd || typeof task.cmd !== 'string') {
      throw new ValidationError('Task must have a valid cmd property');
    }

    if (!test || typeof test !== 'function') {
      throw new ValidationError('Test must be a function');
    }

    const taskWithTest: TaskWithTest = {
      ...task,
      test,
    };

    this.steps.push(taskWithTest);
    return this;
  }

  /**
   * Add multiple tasks to execute in parallel
   * @param tasks - Array of tasks to execute in parallel
   * @returns This TaskBuilder instance for method chaining
   */
  parallel(tasks: Task[]): TaskBuilder {
    if (!Array.isArray(tasks) || tasks.length === 0) {
      throw new ValidationError('Parallel tasks must be a non-empty array');
    }

    // Validate each task
    for (let i = 0; i < tasks.length; i++) {
      if (!tasks[i].cmd || typeof tasks[i].cmd !== 'string') {
        throw new ValidationError(`Task ${i} in parallel step must have a valid cmd property`);
      }
    }

    this.steps.push(tasks);
    return this;
  }

  /**
   * Add multiple tasks with tests to execute in parallel
   * @param tasksWithTests - Array of tasks with tests to execute in parallel
   * @returns This TaskBuilder instance for method chaining
   */
  parallelWithTests(tasksWithTests: TaskWithTest[]): TaskBuilder {
    if (!Array.isArray(tasksWithTests) || tasksWithTests.length === 0) {
      throw new ValidationError('Parallel tasks with tests must be a non-empty array');
    }

    // Validate each task and test
    for (let i = 0; i < tasksWithTests.length; i++) {
      const taskWithTest = tasksWithTests[i];
      if (!taskWithTest.cmd || typeof taskWithTest.cmd !== 'string') {
        throw new ValidationError(`Task ${i} in parallel step must have a valid cmd property`);
      }
      if (taskWithTest.test && typeof taskWithTest.test !== 'function') {
        throw new ValidationError(`Test for task ${i} must be a function`);
      }
    }

    this.steps.push(tasksWithTests);
    return this;
  }

  /**
   * Get the current number of execution steps
   */
  get stepCount(): number {
    return this.steps.length;
  }

  /**
   * Check if the builder is empty (no steps added)
   */
  get isEmpty(): boolean {
    return this.steps.length === 0;
  }

  /**
   * Clear all steps from the builder
   * @returns This TaskBuilder instance for method chaining
   */
  clear(): TaskBuilder {
    this.steps = [];
    return this;
  }

  /**
   * Clone this TaskBuilder
   * @returns A new TaskBuilder with the same steps
   */
  clone(): TaskBuilder {
    const newBuilder = new TaskBuilder();
    newBuilder.steps = [...this.steps];
    return newBuilder;
  }

  /**
   * Build the task group for execution
   * @returns TaskGroup ready for execution
   */
  build(): TaskGroup {
    if (this.steps.length === 0) {
      throw new ValidationError('Cannot build empty task group - at least one step is required');
    }

    return [...this.steps];
  }

  /**
   * Build and validate the task group
   * @returns Validated TaskGroup ready for execution
   */
  buildAndValidate(): TaskGroup {
    const taskGroup = this.build();

    // Additional validation could be added here
    for (let i = 0; i < taskGroup.length; i++) {
      const step = taskGroup[i];
      if (Array.isArray(step)) {
        if (step.length === 0) {
          throw new ValidationError(`Step ${i}: Parallel step cannot be empty`);
        }
      }
    }

    return taskGroup;
  }

  /**
   * Create a TaskBuilder from an existing TaskGroup
   * @param taskGroup - Existing task group to load
   * @returns New TaskBuilder with the task group loaded
   */
  static fromTaskGroup(taskGroup: TaskGroup): TaskBuilder {
    const builder = new TaskBuilder();
    builder.steps = [...taskGroup];
    return builder;
  }

  /**
   * Create a TaskBuilder from a JSON string
   * @param json - JSON string representing a task group
   * @returns New TaskBuilder with the parsed task group loaded
   */
  static fromJSON(json: string): TaskBuilder {
    try {
      const taskGroup = JSON.parse(json) as TaskGroup;
      return TaskBuilder.fromTaskGroup(taskGroup);
    } catch (error) {
      throw new ValidationError(`Failed to parse JSON: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  /**
   * Get a JSON representation of the task group
   * @returns JSON string representing the current task group
   */
  toJSON(): string {
    if (this.steps.length === 0) {
      throw new ValidationError('Cannot serialize empty task builder');
    }

    return JSON.stringify(this.steps);
  }

  /**
   * Get a string representation of the execution plan
   * @returns Human-readable description of the execution plan
   */
  toString(): string {
    if (this.steps.length === 0) {
      return 'Empty task builder';
    }

    const descriptions: string[] = [];

    for (let i = 0; i < this.steps.length; i++) {
      const step = this.steps[i];

      if (Array.isArray(step)) {
        const taskDescriptions = step.map(task => {
          const argsStr = task.args ? ` ${task.args.join(' ')}` : '';
          return `${task.cmd}${argsStr}`;
        });
        descriptions.push(`Step ${i + 1}: PARALLEL [${taskDescriptions.join(', ')}]`);
      } else {
        const argsStr = step.args ? ` ${step.args.join(' ')}` : '';
        descriptions.push(`Step ${i + 1}: SINGLE ${step.cmd}${argsStr}`);
      }
    }

    return descriptions.join('\n');
  }
}