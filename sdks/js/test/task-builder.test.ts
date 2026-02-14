import { describe, it, expect } from 'vitest';
import { TaskBuilder } from '../src/index';
import type { TaskTest } from '../src/index';

describe('TaskBuilder', () => {
  describe('single()', () => {
    it('should add a single task to the builder', () => {
      const builder = new TaskBuilder();
      const task = { cmd: 'echo', args: ['hello'] };

      builder.single(task);

      expect(builder.length).toBe(1);
      expect(builder.build()).toEqual([task]);
    });

    it('should support method chaining', () => {
      const builder = new TaskBuilder();

      const result = builder.single({ cmd: 'echo', args: ['hello'] });

      expect(result).toBe(builder);
    });

    it('should add multiple single tasks in sequence', () => {
      const builder = new TaskBuilder();

      builder
        .single({ cmd: 'echo', args: ['first'] })
        .single({ cmd: 'echo', args: ['second'] })
        .single({ cmd: 'echo', args: ['third'] });

      expect(builder.length).toBe(3);
      expect(builder.build()).toEqual([
        { cmd: 'echo', args: ['first'] },
        { cmd: 'echo', args: ['second'] },
        { cmd: 'echo', args: ['third'] },
      ]);
    });

    it('should preserve task properties', () => {
      const builder = new TaskBuilder();
      const task = {
        cmd: 'gcc',
        args: ['main.c', '-o', 'main'],
        env: { CC: 'gcc', CFLAGS: '-O2' },
        files: { 'main.c': 'int main() { return 0; }' },
        working_dir: '/tmp',
      };

      builder.single(task);

      expect(builder.build()[0]).toEqual(task);
    });
  });

  describe('parallel()', () => {
    it('should add parallel tasks to the builder', () => {
      const builder = new TaskBuilder();
      const tasks = [
        { cmd: 'ls', args: ['-la'] },
        { cmd: 'pwd' },
      ];

      builder.parallel(tasks);

      expect(builder.length).toBe(1);
      expect(builder.build()).toEqual([tasks]);
    });

    it('should support method chaining', () => {
      const builder = new TaskBuilder();

      const result = builder.parallel([{ cmd: 'ls' }]);

      expect(result).toBe(builder);
    });

    it('should add multiple parallel task groups', () => {
      const builder = new TaskBuilder();

      builder
        .parallel([{ cmd: 'echo', args: ['a'] }])
        .parallel([{ cmd: 'echo', args: ['b'] }]);

      expect(builder.length).toBe(2);
    });

    it('should handle empty parallel array', () => {
      const builder = new TaskBuilder();

      builder.parallel([]);

      expect(builder.length).toBe(1);
      expect(builder.build()).toEqual([[]]);
    });
  });

  describe('singleWithTests()', () => {
    it('should add a single task with tests', () => {
      const builder = new TaskBuilder();
      const task = { cmd: 'echo', args: ['hello'] };
      const tests: TaskTest[] = [
        {
          name: 'check stdout',
          assertion: 'equals',
          field: 'stdout',
          expected: 'hello\n',
        },
      ];

      builder.singleWithTests(task, tests);

      expect(builder.length).toBe(1);
      const built = builder.build();
      expect(built[0]).toHaveProperty('tests');
      expect((built[0] as typeof task & { tests: TaskTest[] }).tests).toEqual(tests);
    });

    it('should support method chaining', () => {
      const builder = new TaskBuilder();

      const result = builder.singleWithTests(
        { cmd: 'echo', args: ['hello'] },
        []
      );

      expect(result).toBe(builder);
    });

    it('should preserve task properties along with tests', () => {
      const builder = new TaskBuilder();
      const task = {
        cmd: 'gcc',
        args: ['main.c'],
        env: { CC: 'gcc' },
      };
      const tests: TaskTest[] = [
        { name: 'exit code', assertion: 'equals', field: 'exitCode', expected: 0 },
      ];

      builder.singleWithTests(task, tests);

      const built = builder.build()[0] as typeof task & { tests: TaskTest[] };
      expect(built.cmd).toBe('gcc');
      expect(built.args).toEqual(['main.c']);
      expect(built.env).toEqual({ CC: 'gcc' });
      expect(built.tests).toEqual(tests);
    });
  });

  describe('parallelWithTests()', () => {
    it('should add parallel tasks with tests', () => {
      const builder = new TaskBuilder();
      const tasks = [
        {
          cmd: 'echo',
          args: ['a'],
          tests: [
            { name: 'test a', assertion: 'equals', field: 'stdout', expected: 'a\n' },
          ] as TaskTest[],
        },
        {
          cmd: 'echo',
          args: ['b'],
          tests: [
            { name: 'test b', assertion: 'equals', field: 'stdout', expected: 'b\n' },
          ] as TaskTest[],
        },
      ];

      builder.parallelWithTests(tasks);

      expect(builder.length).toBe(1);
      const built = builder.build();
      expect(Array.isArray(built[0])).toBe(true);
      expect((built[0] as typeof tasks)[0]).toHaveProperty('tests');
    });

    it('should support method chaining', () => {
      const builder = new TaskBuilder();

      const result = builder.parallelWithTests([]);

      expect(result).toBe(builder);
    });
  });

  describe('build()', () => {
    it('should return a copy of the steps array', () => {
      const builder = new TaskBuilder();
      builder.single({ cmd: 'echo', args: ['hello'] });

      const first = builder.build();
      const second = builder.build();

      expect(first).toEqual(second);
      expect(first).not.toBe(second);
    });

    it('should return empty array for new builder', () => {
      const builder = new TaskBuilder();

      expect(builder.build()).toEqual([]);
    });

    it('should return correct structure for mixed steps', () => {
      const builder = new TaskBuilder();

      builder
        .single({ cmd: 'echo', args: ['first'] })
        .parallel([{ cmd: 'ls' }, { cmd: 'pwd' }])
        .single({ cmd: 'echo', args: ['last'] });

      const result = builder.build();
      expect(result).toHaveLength(3);
      expect(Array.isArray(result[0])).toBe(false);
      expect(Array.isArray(result[1])).toBe(true);
      expect(Array.isArray(result[2])).toBe(false);
    });
  });

  describe('length getter', () => {
    it('should return 0 for empty builder', () => {
      const builder = new TaskBuilder();
      expect(builder.length).toBe(0);
    });

    it('should return correct count after adding steps', () => {
      const builder = new TaskBuilder();
      builder.single({ cmd: 'echo', args: ['1'] });
      expect(builder.length).toBe(1);

      builder.parallel([{ cmd: 'ls' }]);
      expect(builder.length).toBe(2);

      builder.single({ cmd: 'echo', args: ['3'] });
      expect(builder.length).toBe(3);
    });
  });

  describe('isEmpty getter', () => {
    it('should return true for new builder', () => {
      const builder = new TaskBuilder();
      expect(builder.isEmpty).toBe(true);
    });

    it('should return false after adding a step', () => {
      const builder = new TaskBuilder();
      builder.single({ cmd: 'echo' });
      expect(builder.isEmpty).toBe(false);
    });

    it('should return true for new builder after checking isEmpty', () => {
      const builder = new TaskBuilder();
      expect(builder.isEmpty).toBe(true);
    });
  });

  describe('iterator', () => {
    it('should allow iteration over steps', () => {
      const builder = new TaskBuilder();
      builder
        .single({ cmd: 'echo', args: ['first'] })
        .parallel([{ cmd: 'ls' }]);

      const steps: unknown[] = [];
      for (const step of builder) {
        steps.push(step);
      }

      expect(steps).toHaveLength(2);
      expect(steps[0]).toEqual({ cmd: 'echo', args: ['first'] });
      expect(Array.isArray(steps[1])).toBe(true);
    });

    it('should work with spread operator', () => {
      const builder = new TaskBuilder();
      builder
        .single({ cmd: 'echo', args: ['a'] })
        .single({ cmd: 'echo', args: ['b'] });

      const steps = [...builder];

      expect(steps).toHaveLength(2);
    });

    it('should work with Array.from', () => {
      const builder = new TaskBuilder();
      builder.single({ cmd: 'echo' });

      const steps = Array.from(builder);

      expect(steps).toHaveLength(1);
    });
  });

  describe('complex workflows', () => {
    it('should handle compile-and-run workflow', () => {
      const builder = new TaskBuilder();

      builder
        .single({
          cmd: 'gcc',
          args: ['main.c', '-o', 'main'],
          files: { 'main.c': 'int main() { return 0; }' },
        })
        .single({ cmd: './main' });

      const steps = builder.build();
      expect(steps).toHaveLength(2);
      expect((steps[0] as { cmd: string }).cmd).toBe('gcc');
      expect((steps[1] as { cmd: string }).cmd).toBe('./main');
    });

    it('should handle parallel test execution workflow', () => {
      const builder = new TaskBuilder();

      builder
        .single({
          cmd: 'npm',
          args: ['install'],
        })
        .parallel([
          { cmd: 'npm', args: ['run', 'test:unit'] },
          { cmd: 'npm', args: ['run', 'test:integration'] },
          { cmd: 'npm', args: ['run', 'lint'] },
        ])
        .single({ cmd: 'npm', args: ['run', 'build'] });

      expect(builder.length).toBe(3);
      const steps = builder.build();
      expect(Array.isArray(steps[1])).toBe(true);
      expect((steps[1] as unknown[]).length).toBe(3);
    });

    it('should handle workflow with tests at each step', () => {
      const builder = new TaskBuilder();

      builder
        .singleWithTests(
          { cmd: 'gcc', args: ['main.c', '-o', 'main'] },
          [
            { name: 'compiles', assertion: 'equals', field: 'exitCode', expected: 0 },
          ]
        )
        .parallelWithTests([
          {
            cmd: './main',
            tests: [
              { name: 'runs', assertion: 'equals', field: 'exitCode', expected: 0 },
            ],
          },
        ]);

      expect(builder.length).toBe(2);
    });
  });
});
