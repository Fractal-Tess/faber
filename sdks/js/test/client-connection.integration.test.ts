import { describe, it, expect, beforeAll } from 'vitest';
import { FaberClient } from '../src/index';
import { getTestConfig } from './setup.integration';

describe('Client Connection Integration Tests', () => {
  let client: FaberClient;
  const config = getTestConfig();

  beforeAll(() => {
    client = new FaberClient({
      baseUrl: config.baseUrl,
      apiKey: config.apiKey,
    });
  });

  describe('Health Check', () => {
    it('should return healthy status', async () => {
      const health = await client.health();
      expect(health.status).toBe('ok');
    });
  });

  describe('Client Configuration', () => {
    it('should execute with valid API key', async () => {
      const result = await client.executeSingle({
        cmd: 'echo',
        args: ['test'],
      });

      expect(result).toBeDefined();
      expect(result.exitCode).toBe(0);
    });

    it('should reject invalid API key', async () => {
      const badClient = new FaberClient({
        baseUrl: config.baseUrl,
        apiKey: 'invalid-key',
      });

      await expect(
        badClient.executeSingle({ cmd: 'echo', args: ['test'] })
      ).rejects.toThrow();
    });
  });
});
