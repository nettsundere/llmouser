import { mkdirSync, writeFileSync } from 'fs'
import { join, resolve } from 'path'
import { test as base, _electron, type ElectronApplication, type Page } from '@playwright/test'
import {
  COVERAGE_ENABLED,
  MAIN_COVERAGE_DIR,
  RENDERER_COVERAGE_DIR
} from './coverage-paths'

interface Fixtures {
  electronApp: ElectronApplication
  page: Page
}

/**
 * Each test gets its own Electron app with:
 *  - LLM_BROWSER_MOCK=1     -> deterministic offline provider
 *  - LLM_BROWSER_USERDATA   -> isolated settings store (per test)
 *  - NODE_V8_COVERAGE       -> main-process V8 coverage dump (when COVERAGE=1)
 * Renderer coverage is collected via the Playwright CDP coverage API.
 */
export const test = base.extend<Fixtures>({
  electronApp: async ({}, use, testInfo) => {
    const userDataDir = join(testInfo.outputDir, 'userData')
    mkdirSync(userDataDir, { recursive: true })

    const env: Record<string, string> = {
      ...(process.env as Record<string, string>),
      LLM_BROWSER_MOCK: '1',
      LLM_BROWSER_USERDATA: userDataDir
    }
    if (COVERAGE_ENABLED) {
      env.NODE_V8_COVERAGE = MAIN_COVERAGE_DIR
    }

    const app = await _electron.launch({ args: [resolve('.')], env })
    await use(app)
    await app.close()
  },

  page: async ({ electronApp }, use, testInfo) => {
    const page = await electronApp.firstWindow()
    await page.waitForLoadState('domcontentloaded')

    if (COVERAGE_ENABLED) {
      await page.coverage.startJSCoverage({ resetOnNavigation: false })
    }

    await use(page)

    if (COVERAGE_ENABLED) {
      const entries = await page.coverage.stopJSCoverage()
      const safeId = testInfo.testId.replace(/[^a-z0-9]/gi, '_')
      writeFileSync(
        join(RENDERER_COVERAGE_DIR, `renderer-${safeId}.json`),
        JSON.stringify(entries)
      )
    }
  }
})

export { expect } from '@playwright/test'
