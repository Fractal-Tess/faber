import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient, TaskBuilder, TestResultAnalyzer } from '../src/index';
import { getTestConfig } from './setup.integration';

describe('Client-Side Testing Integration Tests', () => {
  let client: FaberClient;
  const config = getTestConfig();

  beforeAll(() => {
    client = new FaberClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
    });
  });

  describe('Basic Test Assertions', () => {
    it('should pass equals assertion on exitCode', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['hello'],
          tests: [
            {
              name: 'exit code is 0',
              assertion: 'equals',
              field: 'exitCode',
              expected: 0,
            },
          ],
        },
      ]);

      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults[0].passed).toBe(true);
    });

    it('should pass contains assertion on stdout', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['Hello, World!'],
          tests: [
            {
              name: 'stdout contains greeting',
              assertion: 'contains',
              field: 'stdout',
              expected: 'Hello',
            },
          ],
        },
      ]);

      expect(result.allTestsPassed).toBe(true);
    });

    it('should pass matches assertion with regex', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['test123'],
          tests: [
            {
              name: 'matches pattern',
              assertion: 'matches',
              field: 'stdout',
              expected: /test\d+/,
            },
          ],
        },
      ]);

      expect(result.allTestsPassed).toBe(true);
    });

    it('should fail when assertion does not match', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['actual'],
          tests: [
            {
              name: 'should fail',
              assertion: 'contains',
              field: 'stdout',
              expected: 'expected',
            },
          ],
        },
      ]);

      expect(result.allTestsPassed).toBe(false);
      expect(result.failedCount).toBe(1);
    });
  });

  describe('Multiple Tests per Task', () => {
    it('should run multiple tests on single task', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['Hello, World!'],
          tests: [
            {
              name: 'exit code check',
              assertion: 'equals',
              field: 'exitCode',
              expected: 0,
            },
            {
              name: 'content check',
              assertion: 'contains',
              field: 'stdout',
              expected: 'Hello',
            },
          ],
        },
      ]);

      expect(result.allTestsPassed).toBe(true);
      expect(result.stepResults[0].testResults).toHaveLength(2);
    });
  });

  describe('TestResultAnalyzer', () => {
    it('should analyze successful results', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['test'],
          tests: [
            {
              name: 'check',
              assertion: 'equals',
              field: 'exitCode',
              expected: 0,
            },
          ],
        },
      ]);

      const analyzer = new TestResultAnalyzer(result);
      expect(analyzer.allPassed).toBe(true);
      expect(analyzer.totalSteps).toBe(1);
      expect(analyzer.passedSteps).toBe(1);
      expect(analyzer.failedSteps).toBe(0);
    });

    it('should identify failed steps', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['test'],
          tests: [
            {
              name: 'failing test',
              assertion: 'contains',
              field: 'stdout',
              expected: 'not-present',
            },
          ],
        },
      ]);

      const analyzer = new TestResultAnalyzer(result);
      expect(analyzer.allPassed).toBe(false);
      expect(analyzer.failedSteps).toBe(1);
      
      const failedSteps = analyzer.getFailedSteps();
      expect(failedSteps).toHaveLength(1);
      expect(failedSteps[0].stage).toBe(0);
    });

    it('should format readable report', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['hello'],
          tests: [
            {
              name: 'content check',
              assertion: 'contains',
              field: 'stdout',
              expected: 'hello',
            },
          ],
        },
      ]);

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();
      
      expect(report).toContain('TEST RESULTS');
      expect(report).toContain('ALL PASSED');
    });

    it('should throw on assertAllPassed when tests fail', async () => {
      const result = await client.executeWithTests([
        {
          cmd: 'echo',
          args: ['test'],
          tests: [
            {
              name: 'failing',
              assertion: 'contains',
              field: 'stdout',
              expected: 'missing',
            },
          ],
        },
      ]);

      const analyzer = new TestResultAnalyzer(result);
      expect(() => analyzer.assertAllPassed()).toThrow('Tests failed');
    });
  });

  describe('TaskBuilder with Tests', () => {
    it('should work with TaskBuilder and singleWithTests', async () => {
      const plan = new TaskBuilder().singleWithTests(
        { cmd: 'echo', args: ['hello'] },
        [
          {
            name: 'check output',
            assertion: 'contains',
            field: 'stdout',
            expected: 'hello',
          },
        ]
      );

      const result = await client.executeWithTests(plan);
      expect(result.allTestsPassed).toBe(true);
    });
  });
});
