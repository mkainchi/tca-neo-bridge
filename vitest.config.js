import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// Configuration page tests; run from the repository root (npm test).
export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  test: {
    include: ['web-tests/**/*.test.js'],
    environment: 'jsdom',
    coverage: {
      provider: 'v8',
      include: ['daemon/web/**/*.js'],
      reportsDirectory: 'coverage',
      reporter: ['text'],
      thresholds: { statements: 100, branches: 100, functions: 100, lines: 100 },
    },
  },
});
