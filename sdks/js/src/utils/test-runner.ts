import type { TaskResult } from '../types/execution';
import type { 
  TaskTest, 
  EqualsTest, 
  ContainsTest, 
  MatchesTest,
  TaskTestResult,
  TaskWithTests 
} from '../types/tests';

export function runTests(task: TaskWithTests, result: TaskResult): TaskTestResult[] {
  if (!task.tests || task.tests.length === 0) {
    return [];
  }

  return task.tests.map(test => {
    try {
      switch (test.assertion) {
        case 'equals':
          return runEqualsTest(test, result);
        case 'contains':
          return runContainsTest(test, result);
        case 'matches':
          return runMatchesTest(test, result);
        case 'custom':
          return test.testFn(result);
        default:
          return { 
            name: 'unknown', 
            passed: false, 
            message: `Unknown assertion type` 
          };
      }
    } catch (error) {
      return {
        name: test.name ?? 'unknown',
        passed: false,
        message: `Test threw error: ${error instanceof Error ? error.message : String(error)}`
      };
    }
  });
}

function runEqualsTest(test: EqualsTest, result: TaskResult): TaskTestResult {
  const actual = result[test.field];
  const passed = actual === test.expected;
  return {
    name: test.name,
    passed,
    message: passed 
      ? `${test.field} equals expected value`
      : `Expected ${test.field} to equal "${test.expected}", got "${actual}"`,
    expected: test.expected,
    actual,
  };
}

function runContainsTest(test: ContainsTest, result: TaskResult): TaskTestResult {
  const actual = result[test.field];
  const passed = typeof actual === 'string' && actual.includes(test.expected);
  return {
    name: test.name,
    passed,
    message: passed
      ? `${test.field} contains expected text`
      : `Expected ${test.field} to contain "${test.expected}"`,
    expected: test.expected,
    actual,
  };
}

function runMatchesTest(test: MatchesTest, result: TaskResult): TaskTestResult {
  const actual = result[test.field];
  const passed = typeof actual === 'string' && test.expected.test(actual);
  return {
    name: test.name,
    passed,
    message: passed
      ? `${test.field} matches expected pattern`
      : `Expected ${test.field} to match ${test.expected}`,
    expected: test.expected.toString(),
    actual,
  };
}
