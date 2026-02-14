import { describe, it, expect } from 'vitest';
import { stripTests, stripTestsFromStep } from '../src/index';
import type { TaskWithTests, Task, ExecutionStep } from '../src/index';

describe('stripTests', () => {
  it('should remove tests property from task', () => {
    const task: TaskWithTests = {
      cmd: 'echo',
      args: ['hello'],
      tests: [
        {
          name: 'test stdout',
          assertion: 'equals',
          field: 'stdout',
          expected: 'hello\n',
        },
      ],
    };

    const result = stripTests(task);

    expect(result).not.toHaveProperty('tests');
  });

  it('should preserve all other task properties', () => {
    const task: TaskWithTests = {
      cmd: 'gcc',
      args: ['main.c', '-o', 'main'],
      env: { CC: 'gcc', CFLAGS: '-O2' },
      files: { 'main.c': 'int main() { return 0; }' },
      working_dir: '/tmp',
      tests: [],
    };

    const result = stripTests(task);

    expect(result.cmd).toBe('gcc');
    expect(result.args).toEqual(['main.c', '-o', 'main']);
    expect(result.env).toEqual({ CC: 'gcc', CFLAGS: '-O2' });
    expect(result.files).toEqual({ 'main.c': 'int main() { return 0; }' });
    expect(result.working_dir).toBe('/tmp');
  });

  it('should return task without tests when tests is undefined', () => {
    const task: TaskWithTests = {
      cmd: 'echo',
      args: ['hello'],
    };

    const result = stripTests(task);

    expect(result).not.toHaveProperty('tests');
    expect(result.cmd).toBe('echo');
    expect(result.args).toEqual(['hello']);
  });

  it('should return new object without mutating original', () => {
    const task: TaskWithTests = {
      cmd: 'echo',
      tests: [{ name: 'test', assertion: 'equals', field: 'stdout', expected: '' }],
    };

    const result = stripTests(task);

    expect(result).not.toBe(task);
    expect(task.tests).toBeDefined();
  });

  it('should handle task with only cmd property', () => {
    const task: TaskWithTests = {
      cmd: 'ls',
    };

    const result = stripTests(task);

    expect(result).toEqual({ cmd: 'ls' });
  });

  it('should handle task with empty tests array', () => {
    const task: TaskWithTests = {
      cmd: 'pwd',
      tests: [],
    };

    const result = stripTests(task);

    expect(result).toEqual({ cmd: 'pwd' });
  });
});

describe('stripTestsFromStep', () => {
  it('should strip tests from single task step', () => {
    const step: ExecutionStep = {
      cmd: 'echo',
      args: ['hello'],
      tests: [
        {
          name: 'test',
          assertion: 'equals',
          field: 'stdout',
          expected: 'hello\n',
        },
      ],
    };

    const result = stripTestsFromStep(step);

    expect(result).not.toHaveProperty('tests');
    expect((result as Task).cmd).toBe('echo');
  });

  it('should strip tests from parallel task step', () => {
    const step: ExecutionStep = [
      {
        cmd: 'echo',
        args: ['a'],
        tests: [{ name: 'test a', assertion: 'equals', field: 'stdout', expected: 'a\n' }],
      },
      {
        cmd: 'echo',
        args: ['b'],
        tests: [{ name: 'test b', assertion: 'equals', field: 'stdout', expected: 'b\n' }],
      },
    ];

    const result = stripTestsFromStep(step);

    expect(Array.isArray(result)).toBe(true);
    const tasks = result as Task[];
    expect(tasks).toHaveLength(2);
    expect(tasks[0]).not.toHaveProperty('tests');
    expect(tasks[1]).not.toHaveProperty('tests');
    expect(tasks[0].cmd).toBe('echo');
    expect(tasks[1].cmd).toBe('echo');
  });

  it('should handle single task without tests', () => {
    const step: ExecutionStep = {
      cmd: 'ls',
      args: ['-la'],
    };

    const result = stripTestsFromStep(step);

    expect(result).toEqual({ cmd: 'ls', args: ['-la'] });
  });

  it('should handle parallel tasks without tests', () => {
    const step: ExecutionStep = [
      { cmd: 'pwd' },
      { cmd: 'whoami' },
    ];

    const result = stripTestsFromStep(step);

    expect(result).toEqual([
      { cmd: 'pwd' },
      { cmd: 'whoami' },
    ]);
  });

  it('should handle empty parallel array', () => {
    const step: ExecutionStep = [];

    const result = stripTestsFromStep(step);

    expect(result).toEqual([]);
  });

  it('should handle mixed parallel tasks (some with tests, some without)', () => {
    const step: ExecutionStep = [
      {
        cmd: 'echo',
        tests: [{ name: 'test', assertion: 'equals', field: 'stdout', expected: '' }],
      },
      { cmd: 'ls' },
      {
        cmd: 'pwd',
        tests: [{ name: 'test2', assertion: 'equals', field: 'stdout', expected: '' }],
      },
    ];

    const result = stripTestsFromStep(step);

    const tasks = result as Task[];
    expect(tasks).toHaveLength(3);
    tasks.forEach(task => {
      expect(task).not.toHaveProperty('tests');
    });
  });

  it('should return new object/array without mutating original', () => {
    const step: ExecutionStep = {
      cmd: 'echo',
      tests: [{ name: 'test', assertion: 'equals', field: 'stdout', expected: '' }],
    };

    const result = stripTestsFromStep(step);

    expect(result).not.toBe(step);
    expect((step as TaskWithTests).tests).toBeDefined();
  });

  it('should not mutate original parallel array', () => {
    const step: ExecutionStep = [
      {
        cmd: 'echo',
        tests: [{ name: 'test', assertion: 'equals', field: 'stdout', expected: '' }],
      },
    ];

    const result = stripTestsFromStep(step);

    expect(result).not.toBe(step);
    expect((step as TaskWithTests[])[0].tests).toBeDefined();
  });

  it('should preserve all task properties when stripping tests', () => {
    const step: ExecutionStep = {
      cmd: 'gcc',
      args: ['main.c', '-o', 'main'],
      env: { CC: 'gcc' },
      files: { 'main.c': 'code' },
      working_dir: '/tmp',
      tests: [{ name: 'test', assertion: 'equals', field: 'exitCode', expected: 0 }],
    };

    const result = stripTestsFromStep(step);

    expect(result).toEqual({
      cmd: 'gcc',
      args: ['main.c', '-o', 'main'],
      env: { CC: 'gcc' },
      files: { 'main.c': 'code' },
      working_dir: '/tmp',
    });
  });

  it('should handle complex nested task properties', () => {
    const step: ExecutionStep = [
      {
        cmd: 'npm',
        args: ['install'],
        env: { NODE_ENV: 'production', PATH: '/usr/bin' },
        files: {
          'package.json': '{"name": "test"}',
          '.npmrc': 'registry=...',
        },
        working_dir: '/app',
        tests: [
          { name: 'exit code', assertion: 'equals', field: 'exitCode', expected: 0 },
        ],
      },
      {
        cmd: 'npm',
        args: ['run', 'build'],
        env: { NODE_ENV: 'production' },
        tests: [
          { name: 'no errors', assertion: 'equals', field: 'stderr', expected: '' },
        ],
      },
    ];

    const result = stripTestsFromStep(step);

    const tasks = result as Task[];
    expect(tasks[0].env).toEqual({ NODE_ENV: 'production', PATH: '/usr/bin' });
    expect(tasks[0].files).toEqual({
      'package.json': '{"name": "test"}',
      '.npmrc': 'registry=...',
    });
    expect(tasks[1].args).toEqual(['run', 'build']);
    tasks.forEach(task => {
      expect(task).not.toHaveProperty('tests');
    });
  });
});

describe('task-utils integration', () => {
  it('should correctly strip tests from a complete execution plan', () => {
    const plan: ExecutionStep[] = [
      {
        cmd: 'gcc',
        args: ['main.c', '-o', 'main'],
        files: { 'main.c': 'int main() { return 0; }' },
        tests: [{ name: 'compiles', assertion: 'equals', field: 'exitCode', expected: 0 }],
      },
      [
        {
          cmd: './main',
          tests: [{ name: 'runs', assertion: 'equals', field: 'exitCode', expected: 0 }],
        },
        {
          cmd: 'echo',
          args: ['done'],
        },
      ],
      {
        cmd: 'rm',
        args: ['main'],
      },
    ];

    const cleanedPlan = plan.map(stripTestsFromStep);

    expect(cleanedPlan).toHaveLength(3);

    const singleStep = cleanedPlan[0] as Task;
    expect(singleStep).not.toHaveProperty('tests');
    expect(singleStep.cmd).toBe('gcc');

    const parallelStep = cleanedPlan[1] as Task[];
    expect(Array.isArray(parallelStep)).toBe(true);
    expect(parallelStep).toHaveLength(2);
    parallelStep.forEach(task => {
      expect(task).not.toHaveProperty('tests');
    });

    const lastStep = cleanedPlan[2] as Task;
    expect(lastStep.cmd).toBe('rm');
  });

  it('should produce API-compatible output', () => {
    const step: ExecutionStep = {
      cmd: 'echo',
      args: ['hello'],
      env: { KEY: 'value' },
      tests: [{ name: 'test', assertion: 'equals', field: 'stdout', expected: 'hello\n' }],
    };

    const result = stripTestsFromStep(step);

    expect(result).toStrictEqual({
      cmd: 'echo',
      args: ['hello'],
      env: { KEY: 'value' },
    });
  });
});
