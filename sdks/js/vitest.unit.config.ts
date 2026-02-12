import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts'],
    exclude: ['**/node_modules/**', '**/dist/**', 'test/**/*.integration.test.ts'],
    testTimeout: 10000,
    hookTimeout: 10000,
    globals: true,
    environment: 'node',
  },
});