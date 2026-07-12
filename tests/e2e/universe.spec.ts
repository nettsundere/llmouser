import { test, expect } from './fixtures'

const DEFAULT_UNIVERSE =
  'Our real universe, exactly as it is today: real sites, brands, people and events.'

test('sites are generated in our universe by default', async ({ page }) => {
  await page.getByTestId('settings-button').click()
  await expect(page.getByTestId('settings-universe')).toHaveValue(DEFAULT_UNIVERSE)
  await page.getByTestId('settings-close').click()

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()

  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-universe')).toHaveText(DEFAULT_UNIVERSE)
})

test('redefining the universe alters generation and persists', async ({ page }) => {
  const alternate =
    'A universe where cats achieved sentience in 1932 and run all governments and media.'

  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-universe').fill(alternate)
  await page.getByTestId('settings-save').click()

  await page.getByTestId('address-bar').fill('news.example')
  await page.getByTestId('go-button').click()

  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-universe')).toHaveText(alternate)

  // Survives a renderer reload — stored alongside the other settings.
  await page.reload()
  await page.getByTestId('settings-button').click()
  await expect(page.getByTestId('settings-universe')).toHaveValue(alternate)
})

test('clearing the universe field restores the default', async ({ page }) => {
  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-universe').fill('temporary weird universe')
  await page.getByTestId('settings-save').click()

  await page.getByTestId('settings-button').click()
  await page.getByTestId('settings-universe').fill('')
  await page.getByTestId('settings-save').click()

  await page.getByTestId('settings-button').click()
  await expect(page.getByTestId('settings-universe')).toHaveValue(DEFAULT_UNIVERSE)
})
