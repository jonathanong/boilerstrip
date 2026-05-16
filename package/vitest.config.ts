import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      // index.js is auto-generated platform-dispatch code from napi build;
      // exclude it so coverage reflects only hand-written sources.
      exclude: ['index.js', '**/*.d.ts', 'vitest.config.ts', 'node_modules/**'],
    },
  },
})
