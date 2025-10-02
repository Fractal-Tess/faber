import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['cjs', 'esm', 'iife'],
  dts: true,
  clean: true,
  sourcemap: true,
  minify: false,
  bundle: true,
  splitting: false,
  target: ['es2020', 'node18'],
  platform: 'browser',
  outDir: 'dist',
  external: [],
  esbuildOptions: (options) => {
    options.define = {
      'process.env.NODE_ENV': `"${process.env.NODE_ENV || 'development'}"`,
    };
  },
  banner: {
    js: `/**
 * Faber Runtime SDK - JavaScript/TypeScript Client
 * @version ${process.env.npm_package_version || '0.1.0'}
 * @license MIT
 */`,
  },
});