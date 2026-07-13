import { test, expect } from './fixtures'

const frameSel = '[data-testid="site-frame"]'

test('back and forward restore cached pages', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')
  const forward = page.getByTestId('forward-button')

  await expect(back).toBeDisabled()
  await expect(forward).toBeDisabled()

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(back).toBeDisabled()

  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')
  await expect(back).toBeEnabled()
  await expect(forward).toBeDisabled()

  // Back restores the cached first page: same referer, no regeneration.
  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(frame.getByTestId('mock-referer')).toHaveText('none')
  await expect(page.getByTestId('address-bar')).toHaveValue('first.com')
  await expect(back).toBeDisabled()
  await expect(forward).toBeEnabled()

  await forward.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')
  await expect(frame.getByTestId('mock-referer')).toHaveText('https://first.com')
  await expect(forward).toBeDisabled()
})

test('navigating from mid-history discards forward entries', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')
  const forward = page.getByTestId('forward-button')

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')

  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(forward).toBeEnabled()

  await page.getByTestId('address-bar').fill('third.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(forward).toBeDisabled()

  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(back).toBeDisabled()
})

test('history is tracked per tab', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')
  const forward = page.getByTestId('forward-button')

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')

  // A fresh tab has no history of its own.
  await page.getByTestId('new-tab-button').click()
  await expect(back).toBeDisabled()
  await expect(forward).toBeDisabled()
  await page.getByTestId('address-bar').fill('third.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(back).toBeDisabled()

  // The first tab kept its own stack: back works there.
  await page.getByTestId('tab').nth(0).click()
  await expect(back).toBeEnabled()
  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')

  // The second tab is unaffected by the first tab's back.
  await page.getByTestId('tab').nth(1).click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(back).toBeDisabled()
})

test('link clicks add history entries', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')

  await frame.locator('#mock-link-relative').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com/about')

  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
  await expect(page.getByTestId('address-bar')).toHaveValue('example.com')
})
