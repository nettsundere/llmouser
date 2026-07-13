import { readFileSync } from 'fs'
import { join } from 'path'
import { test, expect } from './fixtures'
import type { ElectronApplication } from '@playwright/test'

/** Trigger File > Save Page as PDF in the application menu. */
function clickSavePdfMenuItem(electronApp: ElectronApplication): Promise<void> {
  return electronApp.evaluate(({ Menu }) => {
    const item = Menu.getApplicationMenu()?.getMenuItemById('save-pdf')
    if (!item) throw new Error('save-pdf menu item not found')
    item.click()
  })
}

test('menu save with no page loaded reports nothing to save', async ({ electronApp, page }) => {
  await clickSavePdfMenuItem(electronApp)
  await expect(page.getByTestId('status')).toHaveText('Nothing to save as PDF')
})

test('saves the current page as a PDF file', async ({ electronApp, page }, testInfo) => {
  const pdfPath = join(testInfo.outputDir, 'example-com.pdf')

  // Playwright can't drive native dialogs; stub the save dialog in main.
  await electronApp.evaluate(({ dialog }, filePath) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath })
  }, pdfPath)

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await expect(page.getByTestId('status')).toHaveText(/Loaded example\.com/)

  await clickSavePdfMenuItem(electronApp)
  await expect(page.getByTestId('status')).toHaveText(/Saved PDF: .*example-com\.pdf/)

  const pdf = readFileSync(pdfPath)
  expect(pdf.subarray(0, 5).toString()).toBe('%PDF-')
})

test('canceling the save dialog leaves the page untouched', async ({ electronApp, page }) => {
  await electronApp.evaluate(({ dialog }) => {
    dialog.showSaveDialog = async () => ({ canceled: true, filePath: '' })
  })

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await expect(page.getByTestId('status')).toHaveText(/Loaded example\.com/)

  await clickSavePdfMenuItem(electronApp)
  await expect(page.getByTestId('status')).toHaveText('PDF save canceled')

  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})
