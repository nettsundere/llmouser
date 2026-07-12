import { test, expect } from './fixtures'

test('generates and renders a page from the address bar', async ({ page }) => {
  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()

  await expect(page.getByTestId('status')).toHaveText(/Loaded example\.com/)

  // Generated HTML lives inside the sandboxed site iframe.
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.locator('#mock-heading')).toContainText('example.com')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})

test('prepends https:// to a bare host', async ({ page }) => {
  await page.getByTestId('address-bar').fill('news.site')
  await page.getByTestId('address-bar').press('Enter')

  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://news.site')
})
