import { describe, it, expect } from 'vitest';
import { runTests } from '../src/index';
import type { TaskWithTests, TaskResult, TaskTest } from '../src/index';

describe('runTests', () => {
  const mockTaskResult = (overrides: Partial<TaskResult> = {}): TaskResult => ({
    stdout: 'hello world\n',
    stderr: '',
    exitCode: 0,
    ...overrides,
  });

  describe('empty tests', () => {
    it('should return empty array when task has no tests', () => {
      const task: TaskWithTests = { cmd: 'echo', args: ['hello'] };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults).toEqual([]);
    });

    it('should return empty array when tests array is empty', () => {
      const task: TaskWithTests = { cmd: 'echo', args: ['hello'], tests: [] };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults).toEqual([]);
    });
  });

  describe('equals assertion', () => {
    it('should pass when stdout equals expected value', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        args: ['hello'],
        tests: [
          {
            name: 'stdout equals hello',
            assertion: 'equals',
            field: 'stdout',
            expected: 'hello world\n',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world\n' });

      const testResults = runTests(task, result);

      expect(testResults).toHaveLength(1);
      expect(testResults[0].passed).toBe(true);
      expect(testResults[0].message).toBe('stdout equals expected value');
    });

    it('should fail when stdout does not equal expected value', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'stdout equals expected',
            assertion: 'equals',
            field: 'stdout',
            expected: 'expected output',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'actual output' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('Expected stdout to equal');
      expect(testResults[0].message).toContain('expected output');
      expect(testResults[0].message).toContain('actual output');
    });

    it('should pass when exitCode equals expected value', () => {
      const task: TaskWithTests = {
        cmd: 'exit',
        args: ['1'],
        tests: [
          {
            name: 'exit code is 1',
            assertion: 'equals',
            field: 'exitCode',
            expected: 1,
          },
        ],
      };
      const result = mockTaskResult({ exitCode: 1 });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
    });

    it('should fail when exitCode does not equal expected value', () => {
      const task: TaskWithTests = {
        cmd: 'exit',
        tests: [
          {
            name: 'exit code check',
            assertion: 'equals',
            field: 'exitCode',
            expected: 0,
          },
        ],
      };
      const result = mockTaskResult({ exitCode: 1 });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].expected).toBe(0);
      expect(testResults[0].actual).toBe(1);
    });

    it('should test stderr field', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'stderr check',
            assertion: 'equals',
            field: 'stderr',
            expected: 'error message',
          },
        ],
      };
      const result = mockTaskResult({ stderr: 'error message' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
    });
  });

  describe('contains assertion', () => {
    it('should pass when stdout contains expected substring', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'stdout contains hello',
            assertion: 'contains',
            field: 'stdout',
            expected: 'hello',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
      expect(testResults[0].message).toBe('stdout contains expected text');
    });

    it('should fail when stdout does not contain expected substring', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'stdout contains expected',
            assertion: 'contains',
            field: 'stdout',
            expected: 'missing',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('Expected stdout to contain');
      expect(testResults[0].message).toContain('missing');
    });

    it('should handle empty string in stdout', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'empty check',
            assertion: 'contains',
            field: 'stdout',
            expected: '',
          },
        ],
      };
      const result = mockTaskResult({ stdout: '' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
    });

    it('should test stderr field', () => {
      const task: TaskWithTests = {
        cmd: 'ls',
        tests: [
          {
            name: 'stderr contains error',
            assertion: 'contains',
            field: 'stderr',
            expected: 'No such file',
          },
        ],
      };
      const result = mockTaskResult({ stderr: 'ls: No such file or directory' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
    });
  });

  describe('matches assertion', () => {
    it('should pass when stdout matches regex pattern', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'matches pattern',
            assertion: 'matches',
            field: 'stdout',
            expected: /hello\s+\w+/,
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
      expect(testResults[0].message).toBe('stdout matches expected pattern');
    });

    it('should fail when stdout does not match regex pattern', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'matches pattern',
            assertion: 'matches',
            field: 'stdout',
            expected: /^\d+$/,
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'not a number' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('Expected stdout to match');
    });

    it('should convert regex to string in expected field', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'regex test',
            assertion: 'matches',
            field: 'stdout',
            expected: /test/,
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'test' });

      const testResults = runTests(task, result);

      expect(testResults[0].expected).toBe('/test/');
    });

    it('should handle complex regex patterns', () => {
      const task: TaskWithTests = {
        cmd: 'gcc',
        tests: [
          {
            name: 'error format',
            assertion: 'matches',
            field: 'stderr',
            expected: /error:.*line \d+/,
          },
        ],
      };
      const result = mockTaskResult({ stderr: 'error: syntax error on line 42' });

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(true);
    });
  });

  describe('custom assertion', () => {
    it('should run custom test function', () => {
      const customTestFn = vi.fn().mockReturnValue({
        name: 'custom test',
        passed: true,
        message: 'custom passed',
      });

      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'custom test',
            assertion: 'custom',
            testFn: customTestFn,
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(customTestFn).toHaveBeenCalledWith(result);
      expect(testResults[0].passed).toBe(true);
      expect(testResults[0].message).toBe('custom passed');
    });

    it('should pass result to custom function', () => {
      const receivedResults: TaskResult[] = [];
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'capture result',
            assertion: 'custom',
            testFn: (result) => {
              receivedResults.push(result);
              return { name: 'capture', passed: true, message: 'captured' };
            },
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'captured output', exitCode: 42 });

      runTests(task, result);

      expect(receivedResults).toHaveLength(1);
      expect(receivedResults[0].stdout).toBe('captured output');
      expect(receivedResults[0].exitCode).toBe(42);
    });

    it('should handle custom test returning failure', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'failing custom',
            assertion: 'custom',
            testFn: () => ({
              name: 'failing custom',
              passed: false,
              message: 'custom failure reason',
            }),
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toBe('custom failure reason');
    });
  });

  describe('multiple tests', () => {
    it('should run multiple tests and return all results', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'test 1',
            assertion: 'equals',
            field: 'stdout',
            expected: 'hello world\n',
          },
          {
            name: 'test 2',
            assertion: 'contains',
            field: 'stdout',
            expected: 'world',
          },
          {
            name: 'test 3',
            assertion: 'equals',
            field: 'exitCode',
            expected: 0,
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world\n' });

      const testResults = runTests(task, result);

      expect(testResults).toHaveLength(3);
      expect(testResults.every(t => t.passed)).toBe(true);
    });

    it('should continue running tests after a failure', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'failing test',
            assertion: 'equals',
            field: 'stdout',
            expected: 'wrong',
          },
          {
            name: 'passing test',
            assertion: 'contains',
            field: 'stdout',
            expected: 'hello',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'hello world' });

      const testResults = runTests(task, result);

      expect(testResults).toHaveLength(2);
      expect(testResults[0].passed).toBe(false);
      expect(testResults[1].passed).toBe(true);
    });
  });

  describe('error handling', () => {
    it('should handle test function throwing error', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'throwing test',
            assertion: 'custom',
            testFn: () => {
              throw new Error('test error');
            },
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('Test threw error');
      expect(testResults[0].message).toContain('test error');
    });

    it('should handle non-Error exceptions', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'throwing string',
            assertion: 'custom',
            testFn: () => {
              throw 'string error';
            },
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('string error');
    });

    it('should handle unknown assertion type', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'unknown test',
            assertion: 'unknown' as 'equals',
            field: 'stdout',
            expected: 'value',
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults[0].passed).toBe(false);
      expect(testResults[0].message).toContain('Unknown assertion type');
    });
  });

  describe('test result structure', () => {
    it('should include test name in result', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'my test name',
            assertion: 'equals',
            field: 'stdout',
            expected: 'hello',
          },
        ],
      };
      const result = mockTaskResult();

      const testResults = runTests(task, result);

      expect(testResults[0].name).toBe('my test name');
    });

    it('should include expected and actual values for equals test', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'equals test',
            assertion: 'equals',
            field: 'stdout',
            expected: 'expected',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'actual' });

      const testResults = runTests(task, result);

      expect(testResults[0].expected).toBe('expected');
      expect(testResults[0].actual).toBe('actual');
    });

    it('should include expected and actual values for contains test', () => {
      const task: TaskWithTests = {
        cmd: 'echo',
        tests: [
          {
            name: 'contains test',
            assertion: 'contains',
            field: 'stdout',
            expected: 'search',
          },
        ],
      };
      const result = mockTaskResult({ stdout: 'actual content' });

      const testResults = runTests(task, result);

      expect(testResults[0].expected).toBe('search');
      expect(testResults[0].actual).toBe('actual content');
    });
  });
});

import { vi } from 'vitest';
