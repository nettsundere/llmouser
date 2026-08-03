import { test, expect } from './fixtures'

test('settings panel has LLM and UX tabs; language lives under UX', async ({ page }) => {
  await page.getByTestId('settings-button').click()

  // LLM tab is active by default and holds the provider fields.
  await expect(page.getByTestId('settings-tab-llm')).toHaveText('LLM Settings')
  await expect(page.getByTestId('settings-tab-ux')).toHaveText('UX')
  await expect(page.getByTestId('settings-provider')).toBeVisible()
  await expect(page.getByTestId('settings-language')).toBeHidden()

  // UX tab shows the language selector and hides the LLM fields.
  await page.getByTestId('settings-tab-ux').click()
  await expect(page.getByTestId('settings-language')).toBeVisible()
  await expect(page.getByTestId('settings-provider')).toBeHidden()

  // Unsaved edits survive a tab switch.
  await page.getByTestId('settings-tab-llm').click()
  await page.getByTestId('settings-model').fill('draft-model')
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-tab-llm').click()
  await expect(page.getByTestId('settings-model')).toHaveValue('draft-model')
})

test('language switch retranslates the whole chrome immediately', async ({ page }) => {
  // Something on screen beyond static chrome: a loaded page with a status line.
  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await expect(page.getByTestId('status')).toHaveText('Loaded example.com')

  await page.getByTestId('settings-button').click()
  await expect(page.getByTestId('settings-title')).toHaveText('Settings')
  await page.getByTestId('settings-tab-ux').click()

  // Each option is labeled in its own language.
  const options = page.getByTestId('settings-language').locator('option')
  await expect(options).toHaveText(['English', 'Русский', '中文'])

  // No Save click: selecting a language applies everywhere at once.
  await page.getByTestId('settings-language').selectOption('ru')
  await expect(page.getByTestId('settings-title')).toHaveText('Настройки')
  await expect(page.getByTestId('settings-tab-llm')).toHaveText('Настройки LLM')
  await expect(page.getByTestId('settings-tab-ux')).toHaveText('Интерфейс')
  await expect(page.getByTestId('address-bar')).toHaveAttribute(
    'placeholder',
    'Введите запрос или адрес сайта'
  )
  await expect(page.getByTestId('new-tab-button')).toHaveAttribute('title', 'Новая вкладка')
  await expect(page.getByTestId('settings-save')).toHaveText('Сохранить')
  // The already-shown status line is retranslated too.
  await expect(page.getByTestId('status')).toHaveText('Загружено: example.com')
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru')

  await page.getByTestId('settings-language').selectOption('zh')
  await expect(page.getByTestId('settings-title')).toHaveText('设置')
  await expect(page.getByTestId('status')).toHaveText('已加载 example.com')

  await page.getByTestId('settings-language').selectOption('en')
  await expect(page.getByTestId('settings-title')).toHaveText('Settings')
  await expect(page.getByTestId('status')).toHaveText('Loaded example.com')
})

test('language switch retranslates the application menu', async ({ electronApp, page }) => {
  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-language').selectOption('ru')
  await expect(page.getByTestId('settings-title')).toHaveText('Настройки')

  const labels = await electronApp.evaluate(({ Menu }) =>
    Menu.getApplicationMenu()?.items.map((item) => item.label)
  )
  expect(labels).toEqual(expect.arrayContaining(['Файл', 'Правка', 'Вид', 'Окно']))

  // Every item inside the menus is translated too, role items included.
  const allItems = await electronApp.evaluate(({ Menu }) =>
    Menu.getApplicationMenu()
      ?.items.flatMap((item) => item.submenu?.items ?? [])
      .map((item) => ({ id: item.id, role: (item as { role?: string }).role, label: item.label }))
  )
  const byId = (id: string) => allItems?.find((item) => item.id === id)?.label
  const byRole = (role: string) => allItems?.find((item) => item.role === role)?.label
  expect(byId('save-pdf')).toBe('Сохранить страницу как PDF…')
  expect(byRole('undo')).toBe('Отменить')
  expect(byRole('copy')).toBe('Копировать')
  expect(byRole('reload')).toBe('Обновить')
  expect(byRole('minimize')).toBe('Свернуть')
  // File > Close Window (macOS) / Exit (elsewhere) — the item the user flagged.
  const closeOrQuit = process.platform === 'darwin' ? byRole('close') : byRole('quit')
  expect(['Закрыть окно', 'Выход']).toContain(closeOrQuit)
})

test('language persists without Save and leaves unsaved edits uncommitted', async ({ page }) => {
  await page.getByTestId('settings-button').click()

  // An unsaved form edit must not be committed by the instant language save.
  await page.getByTestId('settings-model').fill('some-draft-model')
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-language').selectOption('ru')
  await expect(page.getByTestId('settings-title')).toHaveText('Настройки')
  await page.getByTestId('settings-close').click()

  await page.reload()
  // Chrome comes back in Russian even though Save was never pressed.
  await expect(page.getByTestId('address-bar')).toHaveAttribute(
    'placeholder',
    'Введите запрос или адрес сайта'
  )
  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-tab-ux').click()
  await expect(page.getByTestId('settings-language')).toHaveValue('ru')
  await page.getByTestId('settings-tab-llm').click()
  await expect(page.getByTestId('settings-model')).toHaveValue('gpt-4o')

  // Save still persists the language along with everything else.
  await page.getByTestId('settings-tab-ux').click()
  await page.getByTestId('settings-language').selectOption('zh')
  await page.getByTestId('settings-save').click()
  await page.reload()
  await expect(page.getByTestId('address-bar')).toHaveAttribute('placeholder', '搜索或输入网址')
})
