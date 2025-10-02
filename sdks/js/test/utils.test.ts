import { describe, it, expect } from 'vitest';
import {
  getSuccessfulResults,
  getFailedResults,
  allTasksSucceeded,
  anyTaskSucceeded,
  getTotalExecutionTime,
  formatExecutionTime,
  ensureAllTasksSucceeded,
} from '../src/utils';
import type { TaskGroupResult, TaskResult } from '../src/types';

describe('Utility Functions', () => {
  let mockResults: TaskGroupResult;
  let mockTaskResults: TaskResult[];

  beforeEach(() => {
    mockTaskResults = [
      {
        stdout: 'Success 1',
        stderr: '',
        exit_code: 0,
        stats: {
          memory_peak_bytes: 1024,
          cpu_usage_percent: 5.0,
          pids_peak: 1,
          execution_time_ms: 100,
        },
      },
      {
        error: 'Command failed',
        stats: {
          memory_peak_bytes: 512,
          cpu_usage_percent: 2.0,
          pids_peak: 1,
          execution_time_ms: 50,
        },
      },
      {
        stdout: 'Success 2',
        stderr: '',
        exit_code: 0,
        stats: {
          memory_peak_bytes: 2048,
          cpu_usage_percent: 10.0,
          pids_peak: 2,
          execution_time_ms: 200,
        },
      },
    ];

    mockResults = [
      mockTaskResults[0], // Single task
      mockTaskResults,    // Parallel tasks array
      mockTaskResults[2], // Single task
    ];
  });

  describe('getSuccessfulResults', () => {
    it('should extract successful results from mixed results', () => {
      const successful = getSuccessfulResults(mockTaskResults);
      expect(successful).toHaveLength(2);
      expect(successful[0].stdout).toBe('Success 1');
      expect(successful[1].stdout).toBe('Success 2');
    });

    it('should handle all successful results', () => {
      const allSuccessful = mockTaskResults.filter(r => 'stdout' in r);
      const successful = getSuccessfulResults(allSuccessful);
      expect(successful).toHaveLength(2);
    });

    it('should handle all failed results', () => {
      const allFailed = mockTaskResults.filter(r => 'error' in r);
      const successful = getSuccessfulResults(allFailed);
      expect(successful).toHaveLength(0);
    });

    it('should handle empty results', () => {
      const successful = getSuccessfulResults([]);
      expect(successful).toHaveLength(0);
    });

    it('should handle complex TaskGroupResult with nested arrays', () => {
      const successful = getSuccessfulResults(mockResults);
      // mockResults = [SingleTask, [Task1, Task2, Task3], SingleTask]
      // Successful tasks are: SingleTask, Task1, Task3, SingleTask = 4 total
      expect(successful).toHaveLength(4);
      expect(successful[0].stdout).toBe('Success 1');
      expect(successful[1].stdout).toBe('Success 1'); // From parallel array
      expect(successful[3].stdout).toBe('Success 2');
    });
  });

  describe('getFailedResults', () => {
    it('should extract failed results from mixed results', () => {
      const failed = getFailedResults(mockTaskResults);
      expect(failed).toHaveLength(1);
      expect(failed[0].error).toBe('Command failed');
    });

    it('should handle all failed results', () => {
      const allFailed = mockTaskResults.filter(r => 'error' in r);
      const failed = getFailedResults(allFailed);
      expect(failed).toHaveLength(1);
    });

    it('should handle all successful results', () => {
      const allSuccessful = mockTaskResults.filter(r => 'stdout' in r);
      const failed = getFailedResults(allSuccessful);
      expect(failed).toHaveLength(0);
    });

    it('should handle empty results', () => {
      const failed = getFailedResults([]);
      expect(failed).toHaveLength(0);
    });

    it('should handle complex TaskGroupResult with nested arrays', () => {
      const failed = getFailedResults(mockResults);
      // mockResults = [SingleTask, [Task1, Task2, Task3], SingleTask]
      // Failed tasks are: Task2 (from parallel array) = 1 total
      expect(failed).toHaveLength(1);
      expect(failed[0].error).toBe('Command failed');
    });
  });

  describe('allTasksSucceeded', () => {
    it('should return false for mixed results', () => {
      expect(allTasksSucceeded(mockTaskResults)).toBe(false);
    });

    it('should return true for all successful results', () => {
      const allSuccessful = mockTaskResults.filter(r => 'stdout' in r);
      expect(allTasksSucceeded(allSuccessful)).toBe(true);
    });

    it('should return false for all failed results', () => {
      const allFailed = mockTaskResults.filter(r => 'error' in r);
      expect(allTasksSucceeded(allFailed)).toBe(false);
    });

    it('should return true for empty results', () => {
      expect(allTasksSucceeded([])).toBe(true);
    });

    it('should handle TaskGroupResult with nested arrays', () => {
      const mixedTaskGroup = [
        { stdout: 'success', exit_code: 0, stats: { execution_time_ms: 100 } },
        [
          { stdout: 'parallel success 1', exit_code: 0, stats: { execution_time_ms: 50 } },
          { error: 'parallel failed', stats: { execution_time_ms: 75 } },
        ],
        { error: 'failed', stats: { execution_time_ms: 25 } },
      ];

      expect(allTasksSucceeded(mixedTaskGroup)).toBe(false);
    });

    it('should return true for successful TaskGroupResult', () => {
      const successfulTaskGroup = [
        { stdout: 'success', exit_code: 0, stats: { execution_time_ms: 100 } },
        [
          { stdout: 'parallel success 1', exit_code: 0, stats: { execution_time_ms: 50 } },
          { stdout: 'parallel success 2', exit_code: 0, stats: { execution_time_ms: 75 } },
        ],
      ];

      expect(allTasksSucceeded(successfulTaskGroup)).toBe(true);
    });
  });

  describe('anyTaskSucceeded', () => {
    it('should return true for mixed results', () => {
      expect(anyTaskSucceeded(mockTaskResults)).toBe(true);
    });

    it('should return true for all successful results', () => {
      const allSuccessful = mockTaskResults.filter(r => 'stdout' in r);
      expect(anyTaskSucceeded(allSuccessful)).toBe(true);
    });

    it('should return false for all failed results', () => {
      const allFailed = mockTaskResults.filter(r => 'error' in r);
      expect(anyTaskSucceeded(allFailed)).toBe(false);
    });

    it('should return false for empty results', () => {
      expect(anyTaskSucceeded([])).toBe(false);
    });

    it('should handle TaskGroupResult with nested arrays', () => {
      const mixedTaskGroup = [
        { error: 'failed', stats: { execution_time_ms: 100 } },
        [
          { error: 'parallel failed 1', stats: { execution_time_ms: 50 } },
          { error: 'parallel failed 2', stats: { execution_time_ms: 75 } },
        ],
      ];

      expect(anyTaskSucceeded(mixedTaskGroup)).toBe(false);
    });

    it('should return false for all-failed TaskGroupResult', () => {
      const failedTaskGroup = [
        { error: 'failed', stats: { execution_time_ms: 100 } },
        [
          { error: 'parallel failed 1', stats: { execution_time_ms: 50 } },
          { error: 'parallel failed 2', stats: { execution_time_ms: 75 } },
        ],
      ];

      expect(anyTaskSucceeded(failedTaskGroup)).toBe(false);
    });
  });

  describe('getTotalExecutionTime', () => {
    it('should calculate total execution time for mixed results', () => {
      const totalTime = getTotalExecutionTime(mockTaskResults);
      expect(totalTime).toBe(100 + 50 + 200); // Sum of all execution_time_ms
    });

    it('should handle empty results', () => {
      expect(getTotalExecutionTime([])).toBe(0);
    });

    it('should handle TaskGroupResult with nested arrays', () => {
      const totalTime = getTotalExecutionTime(mockResults);
      // mockResults = [SingleTask(100ms), [Task1(100ms), Task2(50ms), Task3(200ms)], SingleTask(200ms)]
      // For parallel step, we take the MAX time: max(100, 50, 200) = 200ms
      // Total = 100 + 200 + 200 = 500ms
      expect(totalTime).toBe(500);
    });

    it('should handle results with missing execution_time', () => {
      const resultsWithMissing = [
        {
          stdout: 'success',
          exit_code: 0,
          stats: { memory_peak_bytes: 1024 }, // Missing execution_time_ms
        },
        {
          stdout: 'success 2',
          exit_code: 0,
          stats: { execution_time_ms: 150 },
        },
      ] as TaskResult[];

      // The implementation will try to access undefined execution_time_ms which results in NaN
      const totalTime = getTotalExecutionTime(resultsWithMissing);
      expect(totalTime).toBeNaN();
    });

    it('should handle zero execution times', () => {
      const zeroTimeResults = [
        { stdout: 'success', exit_code: 0, stats: { execution_time_ms: 0 } },
        { stdout: 'success 2', exit_code: 0, stats: { execution_time_ms: 0 } },
      ] as TaskResult[];

      expect(getTotalExecutionTime(zeroTimeResults)).toBe(0);
    });
  });

  describe('formatExecutionTime', () => {
    it('should format milliseconds correctly', () => {
      expect(formatExecutionTime(500)).toBe('500ms');
      expect(formatExecutionTime(999)).toBe('999ms');
    });

    it('should format seconds correctly', () => {
      expect(formatExecutionTime(1500)).toBe('1.50s');
      expect(formatExecutionTime(30000)).toBe('30.00s');
      expect(formatExecutionTime(59999)).toBe('60.00s');
    });

    it('should format minutes correctly', () => {
      expect(formatExecutionTime(60000)).toBe('1m 0.00s');
      expect(formatExecutionTime(120000)).toBe('2m 0.00s');
      expect(formatExecutionTime(180000)).toBe('3m 0.00s');
    });

    it('should handle mixed units', () => {
      expect(formatExecutionTime(61500)).toBe('1m 1.50s'); // 61.5 seconds
      expect(formatExecutionTime(900)).toBe('900ms');
    });

    it('should handle edge cases', () => {
      expect(formatExecutionTime(0)).toBe('0ms');
      expect(formatExecutionTime(-1)).toBe('-1ms'); // Handle negative values
    });

    it('should handle very large values', () => {
      const largeTime = 2 * 60 * 60 * 1000; // 2 hours in milliseconds
      expect(formatExecutionTime(largeTime)).toBe('120m 0.00s');
    });

    it('should handle decimal values', () => {
      expect(formatExecutionTime(1550)).toBe('1.55s'); // 1.55 seconds
      expect(formatExecutionTime(1549)).toBe('1.55s'); // 1.549 rounds to 1.55
    });
  });

  describe('ensureAllTasksSucceeded', () => {
    it('should not throw for all successful results', () => {
      const allSuccessful = mockTaskResults.filter(r => 'stdout' in r);
      expect(() => ensureAllTasksSucceeded(allSuccessful)).not.toThrow();
    });

    it('should throw error for mixed results', () => {
      expect(() => ensureAllTasksSucceeded(mockTaskResults)).toThrow();
      expect(() => ensureAllTasksSucceeded(mockTaskResults)).toThrow('1 task(s) failed');
    });

    it('should throw error for all failed results', () => {
      const allFailed = mockTaskResults.filter(r => 'error' in r);
      expect(() => ensureAllTasksSucceeded(allFailed)).toThrow();
    });

    it('should not throw for empty results', () => {
      expect(() => ensureAllTasksSucceeded([])).not.toThrow();
    });

    it('should include failure count in error message', () => {
      try {
        ensureAllTasksSucceeded(mockTaskResults);
      } catch (error) {
        expect((error as Error).message).toContain('1 task(s) failed');
      }
    });

    it('should include task details in error message', () => {
      try {
        ensureAllTasksSucceeded(mockTaskResults);
      } catch (error) {
        expect((error as Error).message).toContain('Task 1: Command failed');
      }
    });

    it('should handle TaskGroupResult with nested arrays', () => {
      const mixedTaskGroup = [
        { stdout: 'success', exit_code: 0, stats: { execution_time_ms: 100 } },
        [
          { stdout: 'parallel success 1', exit_code: 0, stats: { execution_time_ms: 50 } },
          { error: 'parallel failed', stats: { execution_time_ms: 75 } },
        ],
      ];

      expect(() => ensureAllTasksSucceeded(mixedTaskGroup)).toThrow();
    });

    it('should not throw for successful TaskGroupResult', () => {
      const successfulTaskGroup = [
        { stdout: 'success', exit_code: 0, stats: { execution_time_ms: 100 } },
        [
          { stdout: 'parallel success 1', exit_code: 0, stats: { execution_time_ms: 50 } },
          { stdout: 'parallel success 2', exit_code: 0, stats: { execution_time_ms: 75 } },
        ],
      ];

      expect(() => ensureAllTasksSucceeded(successfulTaskGroup)).not.toThrow();
    });
  });

  describe('utility function integration', () => {
    it('should work together in common scenarios', () => {
      const results: TaskGroupResult = [
        { stdout: 'Task 1', stderr: '', exit_code: 0, stats: { memory_peak_bytes: 1024, cpu_usage_percent: 5.0, pids_peak: 1, execution_time_ms: 100 } },
        { error: 'Task 2 failed', stats: { memory_peak_bytes: 512, cpu_usage_percent: 2.0, pids_peak: 1, execution_time_ms: 200 } },
        { stdout: 'Task 3', stderr: '', exit_code: 0, stats: { memory_peak_bytes: 2048, cpu_usage_percent: 10.0, pids_peak: 2, execution_time_ms: 150 } },
      ];

      // Get statistics
      const successful = getSuccessfulResults(results);
      const failed = getFailedResults(results);
      const totalTime = getTotalExecutionTime(results);
      const allSucceeded = allTasksSucceeded(results);
      const anySucceeded = anyTaskSucceeded(results);

      expect(successful.length).toBeGreaterThanOrEqual(1);
      expect(failed.length).toBeGreaterThanOrEqual(1);
      expect(totalTime).toBe(450);
      expect(allSucceeded).toBe(false);
      expect(anySucceeded).toBe(true);
      expect(formatExecutionTime(totalTime)).toBe('450ms');

      // Check that ensureAllTasksSucceeded would throw
      expect(() => ensureAllTasksSucceeded(results)).toThrow();
    });
  });
});