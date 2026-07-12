import { test, expect } from './fixtures'

test.beforeEach(async ({ page }) => {
  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})

test('relative link generates the target page instead of loading the chrome', async ({
  page
}) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await frame.locator('#mock-link-relative').click()

  // Regression: previously the relative href resolved against the app's own
  // index.html and the whole chrome (second address bar) loaded inside the iframe.
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com/about')
  await expect(page.getByTestId('address-bar')).toHaveValue('https://example.com/about')
  expect(await page.locator('[data-testid="address-bar"]').count()).toBe(1)
  expect(await frame.locator('[data-testid="address-bar"]').count()).toBe(0)
})

test('absolute link to another site generates that site', async ({ page }) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await frame.locator('#mock-link-absolute').click()

  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.example/page')
  await expect(page.getByTestId('address-bar')).toHaveValue('https://other.example/page')
})

test('JS-driven navigation is blocked and regenerated instead of loading the chrome', async ({
  page
}) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  // location.href bypasses the injected click interceptor; the main process
  // will-frame-navigate handler must catch it.
  await frame.locator('#mock-js-nav').click()

  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com/js-nav')
  await expect(page.getByTestId('address-bar')).toHaveValue('https://example.com/js-nav')
  expect(await page.locator('[data-testid="address-bar"]').count()).toBe(1)
  expect(await frame.locator('[data-testid="address-bar"]').count()).toBe(0)
})

test('hash link stays on the current generated page', async ({ page }) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')
  await frame.locator('#mock-link-hash').click()

  // In-page anchor: no regeneration, same page still shown.
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})
