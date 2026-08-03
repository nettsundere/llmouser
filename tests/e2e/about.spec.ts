import { test, expect } from './fixtures'
import type { ElectronApplication, Page } from '@playwright/test'

async function openAboutWindow(electronApp: ElectronApplication): Promise<Page> {
  const windowPromise = electronApp.waitForEvent('window')
  await electronApp.evaluate(({ Menu }) => {
    Menu.getApplicationMenu()
      ?.items.flatMap((top) => top.submenu?.items ?? [])
      .find((item) => item.id === 'about')
      ?.click()
  })
  const about = await windowPromise
  await about.waitForLoadState('domcontentloaded')
  return about
}

test('About window shows a centered app icon, version and copyright', async ({
  electronApp,
  page
}) => {
  await page.waitForLoadState('domcontentloaded')
  const about = await openAboutWindow(electronApp)

  await expect(about).toHaveTitle('About LLMouser')
  await expect(about.locator('h1')).toHaveText('LLMouser')
  await expect(about.locator('body')).toContainText(/Version \d+\.\d+\.\d+/)
  await expect(about.locator('body')).toContainText('© 2026 Vladimir Kiselev. MIT License')

  // The logo really decoded (a broken image would still occupy layout space).
  const icon = about.locator('img')
  await expect(icon).toBeVisible()
  expect(await icon.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(
    true
  )

  // And it sits in the horizontal center of the window.
  const offCenter = await icon.evaluate((img) => {
    const rect = img.getBoundingClientRect()
    return Math.abs(rect.left + rect.width / 2 - window.innerWidth / 2)
  })
  expect(offCenter).toBeLessThan(2)
})

test('About window follows the UI language', async ({ electronApp, page }) => {
  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-language').selectOption('ru')
  await expect(page.getByTestId('settings-title')).toHaveText('Настройки')

  const about = await openAboutWindow(electronApp)
  await expect(about).toHaveTitle('О программе LLMouser')
  await expect(about.locator('body')).toContainText(/Версия \d+\.\d+\.\d+/)
  await expect(about.locator('body')).toContainText('© 2026 Vladimir Kiselev. MIT License')
})
