import { describe, expect, it, vi } from 'vitest';
import { FaberClient } from '../src/index';

describe('failed API task results', () => {
  it('preserves the error and statistics without inventing completed fields', async () => {
    const fetchFn = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify([{
        error: 'exec failed',
        stats: { outcome: 'infrastructure_failure' },
      }]), { status: 200, headers: { 'Content-Type': 'application/json' } }),
    );
    const client = new FaberClient({
      baseUrl: 'http://faber.invalid',
      apiKey: 'test',
      fetch: fetchFn,
    });

    const result = await client.executeSingle({ cmd: '/missing' });

    expect(result).toEqual({
      error: 'exec failed',
      stats: { outcome: 'infrastructure_failure' },
    });
  });
});
