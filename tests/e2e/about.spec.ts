import { existsSync } from 'fs'
import { test, expect } from './fixtures'

test('about panel carries the app icon, version and copyright', async ({ electronApp }) => {
  // Electron has no getter for about panel options; main exposes them on a global.
  const about = await electronApp.evaluate(
    () =>
      (globalThis as { aboutPanelOptions?: Record<string, string> }).aboutPanelOptions ?? null
  )
  expect(about).not.toBeNull()
  expect(about?.applicationName).toBe('LLMouser')
  expect(about?.copyright).toBe('© 2026 Vladimir Kiselev. MIT License')
  expect(about?.applicationVersion).toMatch(/^\d+\.\d+\.\d+$/)

  // The icon path points at the bundled app icon; same machine, so check it here.
  expect(about?.iconPath).toMatch(/icon(-mac)?\.png$/)
  expect(existsSync(about!.iconPath)).toBe(true)
})
