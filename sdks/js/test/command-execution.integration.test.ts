import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient } from '../src/index';
import { getTestConfig } from './setup.integration';

describe('Command Execution Integration Tests', () => {
  let client: FaberClient;
  const config = getTestConfig();

  beforeAll(() => {
    client = new FaberClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
    });
  });

  describe('Basic Commands', () => {
    it('should execute echo command', async () => {
      const result = await client.executeSingle({
        cmd: 'echo',
        args: ['Hello, World!'],
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Hello, World!');
    });

    it('should capture stdout and stderr separately', async () => {
      const result = await client.executeSingle({
        cmd: 'sh',
        args: ['-c', 'echo "stdout content" && echo "stderr content" >&2'],
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('stdout content');
      expect(result.stderr).toContain('stderr content');
    });

    it('should capture non-zero exit codes', async () => {
      const result = await client.executeSingle({
        cmd: 'sh',
        args: ['-c', 'exit 42'],
      });

      expect(result.exitCode).toBe(42);
    });
  });

  describe('File Operations', () => {
    it('should create and read files', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'cat',
          args: ['test-file.txt'],
          files: {
            'test-file.txt': 'Hello from file!',
          },
        },
      ]);

      const taskResult = result[0];
      if (!Array.isArray(taskResult)) {
        expect(taskResult.exitCode).toBe(0);
        expect(taskResult.stdout).toContain('Hello from file!');
      }
    });

    it('should handle multiple files', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'cat',
          args: ['file1.txt', 'file2.txt'],
          files: {
            'file1.txt': 'Content from file 1\n',
            'file2.txt': 'Content from file 2\n',
          },
        },
      ]);

      const taskResult = result[0];
      if (!Array.isArray(taskResult)) {
        expect(taskResult.exitCode).toBe(0);
        expect(taskResult.stdout).toContain('Content from file 1');
        expect(taskResult.stdout).toContain('Content from file 2');
      }
    });
  });

  describe('Environment Variables', () => {
    it('should set environment variables', async () => {
      const result = await client.executeSingle({
        cmd: 'sh',
        args: ['-c', 'echo $TEST_VAR'],
        env: {
          TEST_VAR: 'test-value',
        },
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('test-value');
    });

    it('should handle multiple environment variables', async () => {
      const result = await client.executeSingle({
        cmd: 'sh',
        args: ['-c', 'echo "$VAR1 $VAR2"'],
        env: {
          VAR1: 'Hello',
          VAR2: 'World',
        },
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Hello World');
    });
  });

  describe('Stdin Handling', () => {
    it('should pass stdin to command', async () => {
      const result = await client.executeSingle({
        cmd: 'cat',
        stdin: 'Hello from stdin!',
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Hello from stdin!');
    });
  });

  describe('Working Directory', () => {
    it('should change working directory', async () => {
      const result = await client.executeSingle({
        cmd: 'pwd',
        working_dir: '/tmp',
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('/tmp');
    });
  });

  describe('Complex Scenarios', () => {
    it('should handle command with args, files, and env', async () => {
      const result = await client.executeSingle({
        cmd: 'sh',
        args: ['-c', 'echo "$GREETING $(cat name.txt)"'],
        files: {
          'name.txt': 'World',
        },
        env: {
          GREETING: 'Hello',
        },
      });

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Hello World');
    });

    it('should handle multi-step pipeline', async () => {
      const result = await client.executeGroup([
        {
          cmd: 'echo',
          args: ['step1'],
        },
        {
          cmd: 'echo',
          args: ['step2'],
        },
        {
          cmd: 'echo',
          args: ['step3'],
        },
      ]);

      expect(result).toHaveLength(3);
      result.forEach((r, i) => {
        if (!Array.isArray(r)) {
          expect(r.exitCode).toBe(0);
          expect(r.stdout).toContain(`step${i + 1}`);
        }
      });
    });
  });
});
