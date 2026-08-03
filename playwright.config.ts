import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e',
  globalSetup: './tests/e2e/global-setup.ts',
  globalTeardown: './tests/e2e/global-teardown.ts',
  // Safe to parallelize: every test launches its own Electron app with an
  // isolated userData dir, and coverage dumps are unique per process (main,
  // NODE_V8_COVERAGE) or per test id (renderer).
  fullyParallel: true,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  reporter: [['list']]
})
