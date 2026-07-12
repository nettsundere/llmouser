import { test, expect } from './fixtures'

test.beforeEach(async ({ page }) => {
  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})

test('generated pages cannot load external resources', async ({ page }) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.locator('#mock-ext-img')).toHaveAttribute('data-net', 'blocked')
})

test('generated pages cannot fetch from the network', async ({ page }) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.locator('#mock-fetch-result')).toHaveText('fetch-blocked')
})

test('the chrome renderer itself cannot reach the network', async ({ page }) => {
  const result = await page.evaluate(() =>
    fetch('https://example.com/').then(
      () => 'ok',
      () => 'blocked'
    )
  )
  expect(result).toBe('blocked')
})
