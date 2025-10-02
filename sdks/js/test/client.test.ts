import { describe, it, expect, beforeEach, vi } from 'vitest';
import { FaberClient } from '../src/client';
import { TaskBuilder } from '../src/task-builder';
import { ValidationError, ApiError, ConnectionError, TimeoutError } from '../src/errors';
import { createMockResponse, mockFetch } from './setup';

describe('FaberClient', () => {
  let client: FaberClient;
  let mockedFetch: any;

  beforeEach(() => {
    vi.clearAllMocks();
    client = new FaberClient({
      baseUrl: 'http://localhost:3000',
      timeout: 5000,
    });
  });

  describe('constructor', () => {
    it('should create a client with default configuration', () => {
      const testClient = new FaberClient({ baseUrl: 'http://test.com' });
      expect(testClient).toBeDefined();
    });

    it('should throw error when baseUrl is missing', () => {
      expect(() => new FaberClient({ baseUrl: '' })).toThrow(ValidationError);
      expect(() => new FaberClient({ baseUrl: undefined as any })).toThrow(ValidationError);
    });

    it('should normalize baseUrl by removing trailing slash', () => {
      const testClient = new FaberClient({ baseUrl: 'http://test.com/' });
      expect((testClient as any).baseUrl).toBe('http://test.com');
    });

    it('should accept API key configuration', () => {
      const testClient = new FaberClient({
        baseUrl: 'http://test.com',
        apiKey: 'test-api-key',
      });
      expect((testClient as any).apiKey).toBe('test-api-key');
    });
  });

  describe('API key authentication', () => {
    beforeEach(() => {
      mockedFetch = mockFetch({ status: 'ok' });
    });

    it('should include API key in Authorization header when provided', async () => {
      const clientWithApiKey = new FaberClient({
        baseUrl: 'http://localhost:3000',
        apiKey: 'test-api-key',
      });

      await clientWithApiKey.health();

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: 'Bearer test-api-key',
          }),
        })
      );
    });

    it('should not include Authorization header when API key is not provided', async () => {
      await client.health();

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.not.objectContaining({
            Authorization: expect.any(String),
          }),
        })
      );
    });

    it('should merge API key with additional headers', async () => {
      const clientWithApiKey = new FaberClient({
        baseUrl: 'http://localhost:3000',
        apiKey: 'test-api-key',
        headers: { 'X-Custom-Header': 'custom-value' },
      });

      await clientWithApiKey.health();

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: 'Bearer test-api-key',
            'X-Custom-Header': 'custom-value',
          }),
        })
      );
    });
  });

  describe('health method', () => {
    beforeEach(() => {
      mockedFetch = mockFetch({ status: 'ok', timestamp: '2023-01-01T00:00:00Z' });
    });

    it('should make GET request to health endpoint', async () => {
      await client.health();

      expect(mockedFetch).toHaveBeenCalledWith(
        'http://localhost:3000/api/v1/health',
        expect.objectContaining({
          method: 'GET',
          headers: expect.objectContaining({
            'Content-Type': 'application/json',
          }),
        })
      );
    });

    it('should return health response data', async () => {
      const healthData = { status: 'ok', timestamp: '2023-01-01T00:00:00Z' };
      mockFetch(healthData);

      const result = await client.health();
      expect(result).toEqual(healthData);
    });

    it('should accept custom timeout', async () => {
      await client.health({ timeout: 1000 });

      // Verify that AbortController is set up with custom timeout
      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          signal: expect.any(AbortSignal),
        })
      );
    });

    it('should accept custom headers', async () => {
      await client.health({ headers: { 'X-Custom': 'value' } });

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-Custom': 'value',
          }),
        })
      );
    });
  });

  describe('executeGroup method', () => {
    const mockTaskGroup = [
      { cmd: 'echo', args: ['hello'] },
    ];

    beforeEach(() => {
      const mockResult = [
        {
          stdout: 'hello\n',
          stderr: '',
          exit_code: 0,
          stats: {
            memory_peak_bytes: 1024,
            cpu_usage_percent: 5.0,
            pids_peak: 1,
            execution_time_ms: 100,
          },
        },
      ];
      mockedFetch = mockFetch(mockResult);
    });

    it('should execute task group successfully', async () => {
      const result = await client.executeGroup(mockTaskGroup);

      expect(mockedFetch).toHaveBeenCalledWith(
        'http://localhost:3000/api/v1/execute',
        expect.objectContaining({
          method: 'POST',
          headers: expect.objectContaining({
            'Content-Type': 'application/json',
          }),
          body: JSON.stringify(mockTaskGroup),
        })
      );
      expect(result).toHaveLength(1);
      expect(result[0]).toHaveProperty('stdout');
    });

    it('should handle TaskBuilder input', async () => {
      const taskBuilder = new TaskBuilder().single({ cmd: 'echo', args: ['hello'] });

      await client.executeGroup(taskBuilder);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify(taskBuilder.build()),
        })
      );
    });

    it('should throw ValidationError for empty task group', async () => {
      await expect(client.executeGroup([])).rejects.toThrow(ValidationError);
      await expect(client.executeGroup([])).rejects.toThrow('Task group cannot be empty');
    });

    it('should throw ValidationError for missing tasks', async () => {
      await expect(client.executeGroup(undefined as any)).rejects.toThrow(ValidationError);
      await expect(client.executeGroup(null as any)).rejects.toThrow(ValidationError);
    });

    it('should accept custom timeout and headers', async () => {
      await client.executeGroup(mockTaskGroup, {
        timeout: 1000,
        headers: { 'X-Custom': 'value' },
      });

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-Custom': 'value',
          }),
          signal: expect.any(AbortSignal),
        })
      );
    });
  });

  describe('executeSingle method', () => {
    beforeEach(() => {
      const mockResult = [
        {
          stdout: 'test output\n',
          stderr: '',
          exit_code: 0,
          stats: {
            memory_peak_bytes: 1024,
            cpu_usage_percent: 5.0,
            pids_peak: 1,
            execution_time_ms: 100,
          },
        },
      ];
      mockedFetch = mockFetch(mockResult);
    });

    it('should execute single task', async () => {
      const task = { cmd: 'echo', args: ['hello'] };
      await client.executeSingle(task);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify([task]),
        })
      );
    });

    it('should handle task with all properties', async () => {
      const task = {
        cmd: 'ls',
        args: ['-la'],
        env: { PATH: '/usr/bin' },
        stdin: 'test input',
        files: { 'test.txt': 'content' },
        working_dir: '/tmp',
      };

      await client.executeSingle(task);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify([task]),
        })
      );
    });

    it('should handle task without args', async () => {
      const task = { cmd: 'pwd' };
      await client.executeSingle(task);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify([task]),
        })
      );
    });

    it('should throw ValidationError for invalid task', async () => {
      const invalidTask = { cmd: '' };

      await expect(client.executeSingle(invalidTask)).rejects.toThrow(ValidationError);
      await expect(client.executeSingle(invalidTask)).rejects.toThrow('Task must have a valid cmd property');
    });

    it('should accept ExecuteOptions', async () => {
      const task = { cmd: 'echo', args: ['hello'] };
      await client.executeSingle(task, {
        timeout: 1000,
        headers: { 'X-Custom': 'value' },
      });

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-Custom': 'value',
          }),
          signal: expect.any(AbortSignal),
        })
      );
    });
  });

  describe('executeParallel method', () => {
    beforeEach(() => {
      const mockResult = [
        [
          {
            stdout: 'output 1\n',
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
            stdout: 'output 2\n',
            stderr: '',
            exit_code: 0,
            stats: {
              memory_peak_bytes: 1024,
              cpu_usage_percent: 5.0,
              pids_peak: 1,
              execution_time_ms: 100,
            },
          },
        ],
      ];
      mockedFetch = mockFetch(mockResult);
    });

    it('should execute tasks in parallel', async () => {
      const tasks = [
        { cmd: 'echo', args: ['1'] },
        { cmd: 'echo', args: ['2'] },
      ];

      await client.executeParallel(tasks);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify([tasks]),
        })
      );
    });

    it('should handle complex tasks in parallel', async () => {
      const tasks = [
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
          env: { PATH: '/usr/bin:/bin' },
        },
      ];

      await client.executeParallel(tasks);

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          body: JSON.stringify([tasks]),
        })
      );
    });

    it('should throw ValidationError for empty tasks array', async () => {
      await expect(client.executeParallel([])).rejects.toThrow(ValidationError);
      await expect(client.executeParallel([])).rejects.toThrow('Tasks must be a non-empty array');
    });

    it('should throw ValidationError for invalid tasks', async () => {
      const invalidTasks = [
        { cmd: 'echo', args: ['1'] },
        { cmd: '' }, // Invalid task
      ];

      await expect(client.executeParallel(invalidTasks)).rejects.toThrow(ValidationError);
      await expect(client.executeParallel(invalidTasks)).rejects.toThrow('Task 1 must have a valid cmd property');
    });

    it('should accept ExecuteOptions', async () => {
      const tasks = [
        { cmd: 'echo', args: ['1'] },
        { cmd: 'echo', args: ['2'] },
      ];

      await client.executeParallel(tasks, {
        timeout: 1000,
        headers: { 'X-Custom': 'value' },
      });

      expect(mockedFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-Custom': 'value',
          }),
          signal: expect.any(AbortSignal),
        })
      );
    });
  });

  
  describe('error handling', () => {
    it('should handle HTTP error responses', async () => {
      const errorResponse = { error: 'Internal server error' };
      const mockedFetch = vi.mocked(fetch);
      mockedFetch.mockResolvedValue({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        json: () => Promise.resolve(errorResponse),
        text: () => Promise.resolve(JSON.stringify(errorResponse)),
      } as Response);

      await expect(client.health()).rejects.toThrow(ApiError);
      await expect(client.health()).rejects.toThrow('HTTP 500: Internal Server Error');
    });

    it('should handle network errors', async () => {
      const mockedFetch = vi.mocked(fetch);
      mockedFetch.mockRejectedValue(new Error('Network error'));

      await expect(client.health()).rejects.toThrow(ConnectionError);
      await expect(client.health()).rejects.toThrow('Failed to connect to Faber runtime');
    });

    it('should handle timeout errors', async () => {
      const mockedFetch = vi.mocked(fetch);
      const abortError = new Error('Request timeout');
      abortError.name = 'AbortError';
      mockedFetch.mockRejectedValue(abortError);

      await expect(client.health({ timeout: 1 })).rejects.toThrow(TimeoutError);
    });

    it('should handle unexpected errors', async () => {
      const mockedFetch = vi.mocked(fetch);
      mockedFetch.mockRejectedValue('Unexpected error');

      await expect(client.health()).rejects.toThrow('Unexpected error');
    });
  });

  describe('withConfig method', () => {
    it('should create new client with different configuration', () => {
      const newClient = client.withConfig({
        baseUrl: 'http://newhost:3000',
        timeout: 10000,
        apiKey: 'new-api-key',
        headers: { 'X-New': 'header' },
      });

      expect(newClient).not.toBe(client);
      expect(newClient).toBeInstanceOf(FaberClient);
    });

    it('should preserve original client configuration', () => {
      const newClient = client.withConfig({});

      expect(newClient).not.toBe(client);
      // Original client should be unchanged
      expect((client as any).baseUrl).toBe('http://localhost:3000');
      expect((client as any).defaultTimeout).toBe(5000);
    });

    it('should merge API key correctly', () => {
      const clientWithKey = new FaberClient({
        baseUrl: 'http://localhost:3000',
        apiKey: 'original-key',
      });

      const newClient = clientWithKey.withConfig({ baseUrl: 'http://newhost:3000' });
      expect((newClient as any).apiKey).toBe('original-key');

      const newClientWithNewKey = clientWithKey.withConfig({ apiKey: 'new-key' });
      expect((newClientWithNewKey as any).apiKey).toBe('new-key');
    });
  });
});