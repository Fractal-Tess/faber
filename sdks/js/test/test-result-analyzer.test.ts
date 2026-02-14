import { describe, it, expect } from 'vitest';
import { TestResultAnalyzer } from '../src/index';
import type {
  ExecutionWithTestsResult,
  SingleStepWithTestsResult,
  ParallelStepWithTestsResult,
  TaskTestResult,
  TaskWithTests,
  TaskResult,
} from '../src/index';

describe('TestResultAnalyzer', () => {
  const mockTaskResult = (overrides: Partial<TaskResult> = {}): TaskResult => ({
    stdout: '',
    stderr: '',
    exitCode: 0,
    ...overrides,
  });

  const mockTestResult = (overrides: Partial<TaskTestResult> = {}): TaskTestResult => ({
    name: 'test',
    passed: true,
    message: 'passed',
    ...overrides,
  });

  const mockSingleStep = (
    overrides: Partial<SingleStepWithTestsResult> = {}
  ): SingleStepWithTestsResult => ({
    stepIndex: 0,
    parallel: false,
    task: { cmd: 'echo' } as TaskWithTests,
    result: mockTaskResult(),
    testResults: [mockTestResult()],
    passed: true,
    ...overrides,
  });

  const mockParallelStep = (
    overrides: Partial<ParallelStepWithTestsResult> = {}
  ): ParallelStepWithTestsResult => ({
    stepIndex: 0,
    parallel: true,
    results: [],
    passed: true,
    ...overrides,
  });

  describe('constructor', () => {
    it('should create analyzer with result', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [],
        allTestsPassed: true,
        passedCount: 0,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer).toBeInstanceOf(TestResultAnalyzer);
    });
  });

  describe('allPassed getter', () => {
    it('should return true when all tests passed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.allPassed).toBe(true);
    });

    it('should return false when any test failed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: false })],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.allPassed).toBe(false);
    });
  });

  describe('totalSteps getter', () => {
    it('should return 0 for empty result', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [],
        allTestsPassed: true,
        passedCount: 0,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.totalSteps).toBe(0);
    });

    it('should return correct count of steps', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0 }),
          mockSingleStep({ stepIndex: 1 }),
          mockParallelStep({ stepIndex: 2 }),
        ],
        allTestsPassed: true,
        passedCount: 3,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.totalSteps).toBe(3);
    });
  });

  describe('passedSteps getter', () => {
    it('should return count of passed steps', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ passed: true }),
          mockSingleStep({ passed: false }),
          mockSingleStep({ passed: true }),
        ],
        allTestsPassed: false,
        passedCount: 2,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.passedSteps).toBe(2);
    });

    it('should return 0 when all steps failed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ passed: false }),
          mockSingleStep({ passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 2,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.passedSteps).toBe(0);
    });
  });

  describe('failedSteps getter', () => {
    it('should return count of failed steps', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ passed: true }),
          mockSingleStep({ passed: false }),
          mockSingleStep({ passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 1,
        failedCount: 2,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.failedSteps).toBe(2);
    });

    it('should return 0 when all steps passed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ passed: true }),
          mockSingleStep({ passed: true }),
        ],
        allTestsPassed: true,
        passedCount: 2,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.failedSteps).toBe(0);
    });
  });

  describe('getFailedSteps()', () => {
    it('should return empty array when all steps passed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.getFailedSteps()).toEqual([]);
    });

    it('should return failed single step info', () => {
      const failedTest = mockTestResult({ passed: false, name: 'failed test' });
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: false,
            task: { cmd: 'gcc', args: ['main.c'] } as TaskWithTests,
            testResults: [failedTest],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const failed = analyzer.getFailedSteps();

      expect(failed).toHaveLength(1);
      expect(failed[0].stage).toBe(0);
      expect(failed[0].type).toBe('single');
      expect(failed[0].cmd).toBe('gcc');
      expect(failed[0].failedTests).toHaveLength(1);
      expect(failed[0].taskIndex).toBeUndefined();
    });

    it('should return failed parallel step info', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockParallelStep({
            stepIndex: 1,
            passed: false,
            results: [
              {
                task: { cmd: 'echo', args: ['a'] } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: true })],
                testsPassed: true,
              },
              {
                task: { cmd: 'echo', args: ['b'] } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: false, name: 'fail' })],
                testsPassed: false,
              },
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const failed = analyzer.getFailedSteps();

      expect(failed).toHaveLength(1);
      expect(failed[0].stage).toBe(1);
      expect(failed[0].type).toBe('parallel');
      expect(failed[0].taskIndex).toBe(1);
      expect(failed[0].cmd).toBe('echo');
    });

    it('should return multiple failed steps', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0, passed: false }),
          mockSingleStep({ stepIndex: 1, passed: true }),
          mockSingleStep({ stepIndex: 2, passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 1,
        failedCount: 2,
      };

      const analyzer = new TestResultAnalyzer(result);
      const failed = analyzer.getFailedSteps();

      expect(failed).toHaveLength(2);
      expect(failed[0].stage).toBe(0);
      expect(failed[1].stage).toBe(2);
    });
  });

  describe('getFirstFailure()', () => {
    it('should return null when all steps passed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.getFirstFailure()).toBeNull();
    });

    it('should return first failed step', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0, passed: false }),
          mockSingleStep({ stepIndex: 1, passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 2,
      };

      const analyzer = new TestResultAnalyzer(result);
      const first = analyzer.getFirstFailure();

      expect(first).not.toBeNull();
      expect(first!.stage).toBe(0);
    });

    it('should return first failed task in parallel step', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockParallelStep({
            stepIndex: 0,
            passed: false,
            results: [
              {
                task: { cmd: 'echo', args: ['first'] } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: false })],
                testsPassed: false,
              },
              {
                task: { cmd: 'echo', args: ['second'] } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: false })],
                testsPassed: false,
              },
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const first = analyzer.getFirstFailure();

      expect(first!.taskIndex).toBe(0);
      expect(first!.cmd).toBe('echo');
    });
  });

  describe('getStageFailure()', () => {
    it('should return null for non-existent stage', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [],
        allTestsPassed: true,
        passedCount: 0,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.getStageFailure(0)).toBeNull();
    });

    it('should return null for passed stage', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ stepIndex: 0, passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.getStageFailure(0)).toBeNull();
    });

    it('should return failure info for failed single stage', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: false,
            task: { cmd: 'gcc' } as TaskWithTests,
            testResults: [mockTestResult({ passed: false })],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const failure = analyzer.getStageFailure(0);

      expect(failure).not.toBeNull();
      expect(failure!.stage).toBe(0);
      expect(failure!.type).toBe('single');
      expect(failure!.cmd).toBe('gcc');
    });

    it('should return failure info for failed parallel stage', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockParallelStep({
            stepIndex: 0,
            passed: false,
            results: [
              {
                task: { cmd: 'ls' } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: true })],
                testsPassed: true,
              },
              {
                task: { cmd: 'pwd' } as TaskWithTests,
                result: mockTaskResult(),
                testResults: [mockTestResult({ passed: false })],
                testsPassed: false,
              },
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const failure = analyzer.getStageFailure(0);

      expect(failure).not.toBeNull();
      expect(failure!.type).toBe('parallel');
      expect(failure!.taskIndex).toBe(1);
      expect(failure!.cmd).toBe('pwd');
    });
  });

  describe('getStageSummary()', () => {
    it('should return summary for single step', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: true,
            testResults: [
              mockTestResult({ passed: true }),
              mockTestResult({ passed: true }),
              mockTestResult({ passed: false }),
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 2,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const summary = analyzer.getStageSummary();

      expect(summary).toHaveLength(1);
      expect(summary[0]).toEqual({
        stage: 0,
        type: 'single',
        passed: true,
        totalTests: 3,
        passedTests: 2,
        failedTests: 1,
      });
    });

    it('should return summary for parallel step', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockParallelStep({
            stepIndex: 0,
            passed: false,
            results: [
              { task: {} as TaskWithTests, result: mockTaskResult(), testResults: [], testsPassed: true },
              { task: {} as TaskWithTests, result: mockTaskResult(), testResults: [], testsPassed: true },
              { task: {} as TaskWithTests, result: mockTaskResult(), testResults: [], testsPassed: false },
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 2,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const summary = analyzer.getStageSummary();

      expect(summary).toHaveLength(1);
      expect(summary[0]).toEqual({
        stage: 0,
        type: 'parallel',
        passed: false,
        totalTasks: 3,
        passedTasks: 2,
        failedTasks: 1,
      });
    });

    it('should return summary for mixed steps', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0, passed: true }),
          mockParallelStep({ stepIndex: 1, passed: true }),
          mockSingleStep({ stepIndex: 2, passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 2,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const summary = analyzer.getStageSummary();

      expect(summary).toHaveLength(3);
      expect(summary[0].type).toBe('single');
      expect(summary[1].type).toBe('parallel');
      expect(summary[2].type).toBe('single');
    });
  });

  describe('formatReport()', () => {
    it('should include header in report', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();

      expect(report).toContain('TEST RESULTS');
      expect(report).toContain('ALL PASSED');
    });

    it('should show failure status when tests failed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: false })],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();

      expect(report).toContain('FAILURES FOUND');
    });

    it('should include stage summary', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0, passed: true }),
          mockParallelStep({ stepIndex: 1, passed: true }),
        ],
        allTestsPassed: true,
        passedCount: 2,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();

      expect(report).toContain('STAGE SUMMARY');
      expect(report).toContain('Stage 0');
      expect(report).toContain('Stage 1');
    });

    it('should include failure details when tests failed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: false,
            task: { cmd: 'gcc' } as TaskWithTests,
            testResults: [
              mockTestResult({ passed: false, name: 'compilation', message: 'failed to compile' }),
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();

      expect(report).toContain('FAILURE DETAILS');
      expect(report).toContain('gcc');
      expect(report).toContain('compilation');
      expect(report).toContain('failed to compile');
    });

    it('should show passed stages when showPassed option is true', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({ stepIndex: 0, passed: true }),
          mockSingleStep({ stepIndex: 1, passed: false }),
        ],
        allTestsPassed: false,
        passedCount: 1,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport({ showPassed: true });

      expect(report).toContain('PASSED STAGES');
      expect(report).toContain('Stage 0');
    });

    it('should include all tests when includeOutput is true', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: false,
            task: { cmd: 'test' } as TaskWithTests,
            testResults: [
              mockTestResult({ passed: true, name: 'passing' }),
              mockTestResult({ passed: false, name: 'failing' }),
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport({ includeOutput: true });

      expect(report).toContain('All Tests');
      expect(report).toContain('passing');
      expect(report).toContain('failing');
    });

    it('should include expected and actual values in failure details', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [
          mockSingleStep({
            stepIndex: 0,
            passed: false,
            task: { cmd: 'echo' } as TaskWithTests,
            testResults: [
              mockTestResult({
                passed: false,
                name: 'output check',
                expected: 'expected',
                actual: 'actual',
              }),
            ],
          }),
        ],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);
      const report = analyzer.formatReport();

      expect(report).toContain('Expected:');
      expect(report).toContain('expected');
      expect(report).toContain('Actual:');
      expect(report).toContain('actual');
    });
  });

  describe('assertAllPassed()', () => {
    it('should not throw when all tests passed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: true })],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(() => analyzer.assertAllPassed()).not.toThrow();
    });

    it('should throw when any test failed', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: false })],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(() => analyzer.assertAllPassed()).toThrow('Tests failed');
    });

    it('should use custom message when provided', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: false })],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(() => analyzer.assertAllPassed('Custom error message')).toThrow(
        'Custom error message'
      );
    });

    it('should include report in error message', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep({ passed: false })],
        allTestsPassed: false,
        passedCount: 0,
        failedCount: 1,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(() => analyzer.assertAllPassed()).toThrow('TEST RESULTS');
    });
  });

  describe('raw getter', () => {
    it('should return the underlying result', () => {
      const result: ExecutionWithTestsResult = {
        results: [],
        stepResults: [mockSingleStep()],
        allTestsPassed: true,
        passedCount: 1,
        failedCount: 0,
      };

      const analyzer = new TestResultAnalyzer(result);

      expect(analyzer.raw).toBe(result);
    });
  });
});
