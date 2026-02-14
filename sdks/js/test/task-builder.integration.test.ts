import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient, TaskBuilder } from '../src/index';
import { getTestConfig } from './setup.integration';

describe('TaskBuilder Integration Tests', () => {
  let client: FaberClient;
  const config = getTestConfig();

  beforeAll(() => {
    client = new FaberClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
    });
  });

  describe('Single Tasks', () => {
    it('should execute single task via TaskBuilder', async () => {
      const plan = new TaskBuilder().single({
        cmd: 'echo',
        args: ['hello from TaskBuilder'],
      });

      const result = await client.executeGroup(plan);

      expect(result).toHaveLength(1);
      const taskResult = result[0];
      if (!Array.isArray(taskResult)) {
        expect(taskResult.exitCode).toBe(0);
        expect(taskResult.stdout).toContain('hello from TaskBuilder');
      }
    });

    it('should execute multiple single tasks in sequence', async () => {
      const plan = new TaskBuilder()
        .single({ cmd: 'echo', args: ['first'] })
        .single({ cmd: 'echo', args: ['second'] })
        .single({ cmd: 'echo', args: ['third'] });

      const result = await client.executeGroup(plan);

      expect(result).toHaveLength(3);
      result.forEach((r) => {
        if (!Array.isArray(r)) {
          expect(r.exitCode).toBe(0);
        }
      });
    });
  });

  describe('Parallel Tasks', () => {
    it('should execute parallel tasks', async () => {
      const plan = new TaskBuilder().parallel([
        { cmd: 'echo', args: ['parallel-a'] },
        { cmd: 'echo', args: ['parallel-b'] },
        { cmd: 'echo', args: ['parallel-c'] },
      ]);

      const result = await client.executeGroup(plan);

      expect(result).toHaveLength(1);
      const parallelResults = result[0];
      if (Array.isArray(parallelResults)) {
        expect(parallelResults).toHaveLength(3);
        parallelResults.forEach((r) => {
          expect(r.exitCode).toBe(0);
        });
      }
    });

    it('should execute mixed sequential and parallel tasks', async () => {
      const plan = new TaskBuilder()
        .single({ cmd: 'echo', args: ['step-1'] })
        .parallel([
          { cmd: 'echo', args: ['para-1'] },
          { cmd: 'echo', args: ['para-2'] },
        ])
        .single({ cmd: 'echo', args: ['step-3'] });

      const result = await client.executeGroup(plan);

      expect(result).toHaveLength(3);
      
      if (!Array.isArray(result[0])) {
        expect(result[0].exitCode).toBe(0);
      }
      
      if (Array.isArray(result[1])) {
        expect(result[1]).toHaveLength(2);
      }
      
      if (!Array.isArray(result[2])) {
        expect(result[2].exitCode).toBe(0);
      }
    });
  });

  describe('TaskBuilder Iteration', () => {
    it('should be iterable', async () => {
      const plan = new TaskBuilder()
        .single({ cmd: 'echo', args: ['a'] })
        .single({ cmd: 'echo', args: ['b'] });

      const steps = [...plan];
      expect(steps).toHaveLength(2);
    });

    it('should build clean array', async () => {
      const plan = new TaskBuilder()
        .single({ cmd: 'echo', args: ['test'] })
        .build();

      expect(Array.isArray(plan)).toBe(true);
      expect(plan).toHaveLength(1);
    });
  });
});
