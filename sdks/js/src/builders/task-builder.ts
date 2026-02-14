import type { ExecutionStep, Task } from '../models';
import type { TaskTest } from '../types/tests';

export type TaskWithTests = Task & {
  tests?: TaskTest[];
};

export class TaskBuilder {
  private steps: ExecutionStep[] = [];

  single(task: Task): this {
    this.steps.push(task);
    return this;
  }

  parallel(tasks: Task[]): this {
    this.steps.push(tasks);
    return this;
  }

  singleWithTests(task: Task, tests: TaskTest[]): this {
    const taskWithTests: TaskWithTests = { ...task, tests };
    this.steps.push(taskWithTests);
    return this;
  }

  parallelWithTests(tasks: TaskWithTests[]): this {
    this.steps.push(tasks);
    return this;
  }

  build(): ExecutionStep[] {
    return [...this.steps];
  }

  get length(): number {
    return this.steps.length;
  }

  get isEmpty(): boolean {
    return this.steps.length === 0;
  }

  *[Symbol.iterator](): Iterator<ExecutionStep> {
    for (const step of this.steps) {
      yield step;
    }
  }
}
