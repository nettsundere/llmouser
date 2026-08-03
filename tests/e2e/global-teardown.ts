import { existsSync, readdirSync, readFileSync } from 'fs'
import { join } from 'path'
import { CoverageReport } from 'monocart-coverage-reports'
import { COVERAGE_OUTPUT_DIR, MAIN_COVERAGE_DIR, RENDERER_COVERAGE_DIR } from './coverage-paths'

export default async function globalTeardown(): Promise<void> {
  const report = new CoverageReport({
    name: 'LLM Browser E2E Coverage',
    outputDir: COVERAGE_OUTPUT_DIR,
    reports: ['v8', 'lcovonly', 'console-summary'],
    // Keep only this app's own source, drop node_modules / electron internals.
    entryFilter: (entry: { url: string }) =>
      !entry.url.includes('node_modules') && !entry.url.startsWith('node:'),
    sourceFilter: (path: string) => path.includes('src/') && !path.includes('node_modules')
  })

  // Main-process V8 coverage (one JSON per process, dumped via NODE_V8_COVERAGE).
  if (existsSync(MAIN_COVERAGE_DIR)) {
    await report.addFromDir(MAIN_COVERAGE_DIR)
  }

  // Renderer coverage collected per test through the Playwright CDP API.
  if (existsSync(RENDERER_COVERAGE_DIR)) {
    for (const file of readdirSync(RENDERER_COVERAGE_DIR)) {
      if (!file.endsWith('.json')) continue
      const entries = JSON.parse(readFileSync(join(RENDERER_COVERAGE_DIR, file), 'utf-8'))
      // Tests that never execute renderer JS dump an empty list; monocart rejects it.
      if (Array.isArray(entries) && entries.length > 0) await report.add(entries)
    }
  }

  await report.generate()
}
