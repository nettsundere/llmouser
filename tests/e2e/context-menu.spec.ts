import { test, expect } from './fixtures'
import type { ElectronApplication } from '@playwright/test'

interface CapturedItem {
  id?: string
  role?: string
  label?: string
  type?: string
  enabled?: boolean
}

/**
 * The context menu is a native popup Playwright cannot see. Instead, intercept
 * Menu.buildFromTemplate, synthesize a `context-menu` event on the webContents
 * (suppressing the real popup), and inspect the captured template.
 */
function captureContextMenu(
  electronApp: ElectronApplication,
  editFlags: { canCut?: boolean; canCopy?: boolean; canPaste?: boolean },
  clickSavePdf = false
): Promise<CapturedItem[] | null> {
  return electronApp.evaluate(
    ({ Menu, BrowserWindow }, { editFlags, clickSavePdf }) => {
      const win = BrowserWindow.getAllWindows()[0]
      let captured: CapturedItem[] | null = null
      let savePdfClick: (() => void) | undefined

      const original = Menu.buildFromTemplate.bind(Menu)
      Menu.buildFromTemplate = ((template: Electron.MenuItemConstructorOptions[]) => {
        captured = template.map((item) => ({
          id: item.id,
          role: item.role as string | undefined,
          label: item.label,
          type: item.type as string | undefined,
          enabled: item.enabled
        }))
        const savePdf = template.find((item) => item.id === 'save-pdf')
        savePdfClick = savePdf?.click as (() => void) | undefined
        const menu = original(template)
        menu.popup = () => {} // keep the native menu closed under test
        return menu
      }) as typeof Menu.buildFromTemplate

      win.webContents.emit('context-menu', {}, { x: 10, y: 10, editFlags })
      Menu.buildFromTemplate = original

      if (clickSavePdf) savePdfClick?.()
      return captured as CapturedItem[] | null
    },
    { editFlags, clickSavePdf }
  )
}

test('right-click offers cut/copy/paste honoring edit state, plus Save as PDF', async ({
  electronApp,
  page
}) => {
  await page.waitForLoadState('domcontentloaded')
  const items = await captureContextMenu(electronApp, {
    canCut: false,
    canCopy: true,
    canPaste: true
  })

  expect(items?.map((item) => item.role ?? item.id)).toEqual([
    'cut',
    'copy',
    'paste',
    undefined, // separator
    'save-pdf'
  ])
  const byRole = (role: string) => items?.find((item) => item.role === role)
  expect(byRole('cut')?.label).toBe('Cut')
  expect(byRole('cut')?.enabled).toBe(false)
  expect(byRole('copy')?.label).toBe('Copy')
  expect(byRole('copy')?.enabled).toBe(true)
  expect(byRole('paste')?.label).toBe('Paste')
  expect(byRole('paste')?.enabled).toBe(true)
  expect(items?.find((item) => item.id === 'save-pdf')?.label).toBe('Save Page as PDF…')
})

test('context menu follows the UI language', async ({ electronApp, page }) => {
  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-language').selectOption('ru')
  await expect(page.getByTestId('settings-title')).toHaveText('Настройки')

  const items = await captureContextMenu(electronApp, { canCopy: true })
  const labels = items?.map((item) => item.label)
  expect(labels).toEqual(
    expect.arrayContaining(['Вырезать', 'Копировать', 'Вставить', 'Сохранить страницу как PDF…'])
  )
})

test('context menu Save as PDF pings the renderer like the app menu does', async ({
  electronApp,
  page
}) => {
  // No page loaded: the renderer answers the ping with its "nothing to save" status.
  await captureContextMenu(electronApp, {}, true)
  await expect(page.getByTestId('status')).toHaveText('Nothing to save as PDF')
})
