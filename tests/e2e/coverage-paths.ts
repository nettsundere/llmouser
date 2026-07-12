import { resolve } from 'path'

/** Coverage collection is opt-in via COVERAGE=1 (set by the test:e2e:coverage script). */
export const COVERAGE_ENABLED = process.env.COVERAGE === '1'

const RAW_ROOT = resolve('.coverage-raw')

/** Main-process V8 coverage is dumped here via NODE_V8_COVERAGE. */
export const MAIN_COVERAGE_DIR = resolve(RAW_ROOT, 'main')

/** Renderer JS coverage (from Playwright/CDP) is written here per test. */
export const RENDERER_COVERAGE_DIR = resolve(RAW_ROOT, 'renderer')

export const RAW_COVERAGE_ROOT = RAW_ROOT

/** Final report output. */
export const COVERAGE_OUTPUT_DIR = resolve('coverage')
