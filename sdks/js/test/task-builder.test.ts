import { describe, it, expect, beforeEach } from 'vitest';
import { TaskBuilder } from '../src/task-builder';
import { ValidationError } from '../src/errors';
import type { Task, TaskGroup } from '../src/types';

describe('TaskBuilder', () => {
  let builder: TaskBuilder;

  beforeEach(() => {
    builder = new TaskBuilder();
  });

  describe('constructor', () => {
    it('should create an empty TaskBuilder', () => {
      expect(builder).toBeDefined();
      // Empty TaskBuilder should throw when build() is called
      expect(() => builder.build()).toThrow('Cannot build empty task group');
    });
  });

  describe('single method', () => {
    it('should add a single task', () => {
      const task: Task = { cmd: 'echo', args: ['hello'] };
      builder.single(task);

      const result = builder.build();
      expect(result).toHaveLength(1);
      expect(result[0]).toEqual(task);
    });

    it('should throw ValidationError for empty task', () => {
      expect(() => builder.single({} as Task)).toThrow(ValidationError);
      expect(() => builder.single({} as Task)).toThrow('Task must have a valid cmd property');
    });

    it('should throw ValidationError for invalid cmd', () => {
      expect(() => builder.single({ cmd: '' } as Task)).toThrow(ValidationError);
      expect(() => builder.single({ cmd: null as any })).toThrow(ValidationError);
      expect(() => builder.single({ cmd: 123 as any })).toThrow(ValidationError);
    });

    // Note: TaskBuilder only validates cmd property, not other properties
    it('should accept tasks with valid cmd but other properties', () => {
      expect(() => {
        builder.single({
          cmd: 'test',
          args: 'not-array' as any,
          env: 'not-object' as any,
          files: 'not-object' as any,
        });
      }).not.toThrow();
    });

    it('should return builder instance for chaining', () => {
      const task: Task = { cmd: 'echo' };
      const result = builder.single(task);
      expect(result).toBe(builder);
    });

    it('should allow multiple single tasks', () => {
      const task1: Task = { cmd: 'echo', args: ['hello'] };
      const task2: Task = { cmd: 'pwd' };

      builder.single(task1).single(task2);

      const result = builder.build();
      expect(result).toHaveLength(2);
      expect(result[0]).toEqual(task1);
      expect(result[1]).toEqual(task2);
    });

    it('should validate optional fields', () => {
      const task: Task = {
        cmd: 'test',
        args: ['arg1', 'arg2'],
        env: { VAR: 'value' },
        stdin: 'input',
        files: { 'file.txt': 'content' },
        working_dir: '/tmp',
      };

      builder.single(task);
      const result = builder.build();
      expect(result[0]).toEqual(task);
    });
  });

  describe('parallel method', () => {
    it('should add parallel tasks', () => {
      const tasks: Task[] = [
        { cmd: 'echo', args: ['1'] },
        { cmd: 'echo', args: ['2'] },
      ];
      builder.parallel(tasks);

      const result = builder.build();
      expect(result).toHaveLength(1);
      expect(result[0]).toEqual(tasks);
    });

    it('should throw ValidationError for empty tasks array', () => {
      expect(() => builder.parallel([])).toThrow(ValidationError);
      expect(() => builder.parallel([])).toThrow('Parallel tasks must be a non-empty array');
    });

    it('should throw ValidationError for invalid tasks in array', () => {
      const tasks = [
        { cmd: 'echo', args: ['1'] },
        { cmd: '' }, // Invalid task
      ];

      expect(() => builder.parallel(tasks)).toThrow(ValidationError);
    });

    it('should validate all tasks in parallel array', () => {
      const tasks = [
        { cmd: 'echo', args: ['1'] },
        { cmd: null as any }, // Invalid
        { cmd: 'pwd' },
      ];

      expect(() => builder.parallel(tasks)).toThrow(ValidationError);
      expect(() => builder.parallel(tasks)).toThrow('Task 1 in parallel step must have a valid cmd property');
    });

    it('should return builder instance for chaining', () => {
      const tasks: Task[] = [{ cmd: 'echo' }];
      const result = builder.parallel(tasks);
      expect(result).toBe(builder);
    });

    it('should allow complex parallel tasks with all fields', () => {
      const tasks: Task[] = [
        {
          cmd: 'gcc',
          args: ['file.c', '-o', 'file'],
          env: { CC: 'gcc' },
          files: { 'file.c': 'int main() { return 0; }' },
        },
        {
          cmd: 'make',
          args: ['-j2'],
          working_dir: '/build',
        },
      ];

      builder.parallel(tasks);
      const result = builder.build();
      expect(result[0]).toEqual(tasks);
    });
  });

  describe('build method', () => {
    it('should return a copy of the tasks', () => {
      const task: Task = { cmd: 'echo' };
      builder.single(task);

      const result1 = builder.build();
      const result2 = builder.build();

      expect(result1).toEqual(result2);
      expect(result1).not.toBe(result2); // Should be different objects
    });

    it('should throw for no tasks', () => {
      const emptyBuilder = new TaskBuilder();
      expect(() => emptyBuilder.build()).toThrow('Cannot build empty task group');
    });

    it('should preserve task order', () => {
      const task1: Task = { cmd: 'echo', args: ['1'] };
      const task2: Task = { cmd: 'echo', args: ['2'] };
      const tasks: Task[] = [
        { cmd: 'echo', args: ['3'] },
        { cmd: 'echo', args: ['4'] },
      ];

      builder.single(task1).parallel(tasks).single(task2);

      const result = builder.build();
      expect(result).toHaveLength(3);
      expect(result[0]).toEqual(task1);
      expect(result[1]).toEqual(tasks);
      expect(result[2]).toEqual(task2);
    });

    it('should handle complex mixed execution pattern', () => {
      // Step 1: Single task - compile
      builder.single({
        cmd: 'gcc',
        args: ['main.c', '-o', 'main'],
        env: { CFLAGS: '-O2' },
        files: {
          'main.c': `
            #include <stdio.h>
            int main() {
              printf("Hello World\\n");
              return 0;
            }
          `,
        },
      });

      // Step 2: Parallel tasks - run multiple instances
      builder.parallel([
        { cmd: './main', stdin: 'Alice\\n' },
        { cmd: './main', stdin: 'Bob\\n' },
        { cmd: './main', stdin: 'Charlie\\n' },
      ]);

      // Step 3: Single task - cleanup
      builder.single({ cmd: 'rm', args: ['main'] });

      const result = builder.build();
      expect(result).toHaveLength(3);

      // Verify structure
      expect(Array.isArray(result[0])).toBe(false); // Single task
      expect(Array.isArray(result[1])).toBe(true);  // Parallel tasks
      expect(Array.isArray(result[2])).toBe(false); // Single task

      // Verify parallel step has 3 tasks
      expect(result[1]).toHaveLength(3);
    });
  });

  describe('clone method', () => {
    it('should create a copy of the builder', () => {
      const task: Task = { cmd: 'echo' };
      builder.single(task);

      const cloned = builder.clone();

      expect(cloned).not.toBe(builder);
      expect(cloned.build()).toEqual(builder.build());
    });

    it('should create independent builder', () => {
      const task: Task = { cmd: 'echo' };
      builder.single(task);

      const cloned = builder.clone();
      cloned.single({ cmd: 'pwd' });

      expect(builder.build()).toHaveLength(1);
      expect(cloned.build()).toHaveLength(2);
    });
  });

  describe('clear method', () => {
    it('should remove all tasks', () => {
      builder.single({ cmd: 'echo' }).parallel([{ cmd: 'pwd' }]);
      expect(builder.build()).toHaveLength(2);

      builder.clear();
      expect(() => builder.build()).toThrow('Cannot build empty task group');
    });

    it('should return builder instance for chaining', () => {
      const result = builder.clear();
      expect(result).toBe(builder);
    });
  });

  describe('toString method', () => {
    it('should return string representation', () => {
      builder.single({ cmd: 'echo', args: ['hello'] });
      builder.parallel([{ cmd: 'pwd' }, { cmd: 'ls' }]);

      const str = builder.toString();
      expect(typeof str).toBe('string');
      expect(str).toContain('echo');
      expect(str).toContain('pwd');
      expect(str).toContain('ls');
    });

    it('should handle empty builder', () => {
      const str = builder.toString();
      expect(typeof str).toBe('string');
      expect(str).toBe('Empty task builder');
    });
  });

  describe('toJSON method', () => {
    it('should return JSON representation', () => {
      const task: Task = { cmd: 'echo', args: ['hello'] };
      builder.single(task);

      const json = builder.toJSON();
      const parsed = JSON.parse(json);
      expect(parsed).toEqual([task]);
    });

    it('should throw for empty builder', () => {
      expect(() => builder.toJSON()).toThrow('Cannot serialize empty task builder');
    });
  });

  describe('static methods', () => {
    describe('fromTaskGroup', () => {
      it('should create TaskBuilder from existing TaskGroup', () => {
        const taskGroup: TaskGroup = [
          { cmd: 'echo', args: ['1'] },
          [{ cmd: 'pwd' }, { cmd: 'ls' }],
          { cmd: 'date' },
        ];

        const newBuilder = TaskBuilder.fromTaskGroup(taskGroup);
        expect(newBuilder.build()).toEqual(taskGroup);
      });

      it('should create independent TaskBuilder', () => {
        const taskGroup: TaskGroup = [{ cmd: 'echo' }];
        const newBuilder = TaskBuilder.fromTaskGroup(taskGroup);

        newBuilder.single({ cmd: 'pwd' });
        expect(taskGroup).toHaveLength(1); // Original unchanged
        expect(newBuilder.build()).toHaveLength(2); // New builder modified
      });

      it('should handle empty TaskGroup', () => {
        const newBuilder = TaskBuilder.fromTaskGroup([]);
        // The fromTaskGroup method might not throw, but build() will
        expect(() => newBuilder.build()).toThrow('Cannot build empty task group');
      });
    });

    describe('fromJSON', () => {
      it('should create TaskBuilder from JSON string', () => {
        const taskGroup: TaskGroup = [
          { cmd: 'echo', args: ['hello'] },
          [{ cmd: 'pwd' }, { cmd: 'ls' }],
        ];
        const json = JSON.stringify(taskGroup);

        const newBuilder = TaskBuilder.fromJSON(json);
        expect(newBuilder.build()).toEqual(taskGroup);
      });

      it('should throw ValidationError for invalid JSON', () => {
        expect(() => TaskBuilder.fromJSON('invalid json')).toThrow(ValidationError);
        expect(() => TaskBuilder.fromJSON('invalid json')).toThrow('Failed to parse JSON');
      });

      it('should handle empty JSON array', () => {
        const newBuilder = TaskBuilder.fromJSON('[]');
        // The fromJSON method might not throw, but build() will
        expect(() => newBuilder.build()).toThrow('Cannot build empty task group');
      });
    });
  });

  describe('chaining behavior', () => {
    it('should support method chaining', () => {
      const result = builder
        .single({ cmd: 'echo', args: ['step1'] })
        .parallel([{ cmd: 'pwd' }, { cmd: 'ls' }])
        .single({ cmd: 'date' });

      expect(result).toBe(builder);
      expect(builder.build()).toHaveLength(3);
    });

    it('should validate at each step', () => {
      expect(() => {
        builder
          .single({ cmd: 'echo' })
          .single({ cmd: '' }) // This should throw
          .single({ cmd: 'date' });
      }).toThrow(ValidationError);
    });
  });

  describe('edge cases', () => {
    it('should handle tasks with complex arguments', () => {
      const complexTask: Task = {
        cmd: 'command',
        args: ['arg with spaces', 'arg-with-special-chars!@#', '"quoted arg"'],
        env: {
          'COMPLEX_VAR': 'value with spaces & symbols',
          'PATH': '/usr/bin:/bin',
        },
        files: {
          'file with spaces.txt': 'content with "quotes" and \n newlines',
          'special-chars!@#.txt': 'content',
        },
        stdin: 'input with special characters: !@#$%^&*()',
        working_dir: '/path with spaces',
      };

      builder.single(complexTask);
      const result = builder.build();
      expect(result[0]).toEqual(complexTask);
    });

    it('should handle tasks with empty strings and zero values', () => {
      const task: Task = {
        cmd: 'command',
        args: [''],
        env: { EMPTY_VAR: '' },
        stdin: '',
        files: { 'empty.txt': '' },
      };

      builder.single(task);
      const result = builder.build();
      expect(result[0]).toEqual(task);
    });

    it('should handle very large task groups', () => {
      const manyTasks: Task[] = Array.from({ length: 1000 }, (_, i) => ({
        cmd: 'echo',
        args: [`task-${i}`],
      }));

      builder.parallel(manyTasks);
      const result = builder.build();
      expect(result[0]).toHaveLength(1000);
    });
  });
});