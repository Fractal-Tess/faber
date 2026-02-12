import type { ExecutionStep, Task } from '../models';
import type { TaskWithTests } from '../types/tests';

export function stripTests(task: TaskWithTests): Task {
  const { tests, ...cleanTask } = task;
  return cleanTask;
}

export function stripTestsFromStep(step: ExecutionStep): ExecutionStep {
  if (Array.isArray(step)) {
    return step.map(stripTests);
  }
  return stripTests(step as TaskWithTests);
}
