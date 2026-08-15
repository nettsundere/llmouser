import { test, expect } from './fixtures'

const frameSel = '[data-testid="site-frame"]'

// The mock provider delays "slow" URLs by 3s, so these tests can press Back
// while the generation is still in flight. If cancellation were broken, the
// slow result would land after the delay and clobber the page — the
// post-delay assertions below would fail.
const SLOW_DELAY_MS = 3500

test('back during load, then a new request: the old request is cancelled', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')
  const forward = page.getByTestId('forward-button')
  const status = page.getByTestId('status')

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')

  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')

  // Start a navigation that stays in flight.
  await page.getByTestId('address-bar').fill('slow.com')
  await page.getByTestId('go-button').click()
  await expect(status).toHaveText('Loading slow.com…')

  // Back while it is loading restores the cached page.
  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(status).toHaveText('Loaded first.com')

  // Another request supersedes the (cancelled) slow one.
  await page.getByTestId('address-bar').fill('third.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(forward).toBeDisabled()

  // Past the point where the slow result would have arrived: nothing changed.
  await page.waitForTimeout(SLOW_DELAY_MS)
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(page.getByTestId('address-bar')).toHaveValue('third.com')
  await expect(status).toHaveText('Loaded third.com')
  await expect(forward).toBeDisabled()

  // History is [first, third] — the slow page never became an entry.
  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(back).toBeDisabled()
})

test('concurrent requests in different tabs do not cancel each other', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const status = page.getByTestId('status')

  // Tab 1: start a slow generation and leave it in flight.
  await page.getByTestId('address-bar').fill('slow.com')
  await page.getByTestId('go-button').click()
  await expect(status).toHaveText('Loading slow.com…')

  // Tab 2: a second request while tab 1 is still generating.
  await page.getByTestId('new-tab-button').click()
  await page.getByTestId('address-bar').fill('other.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.com')

  // Tab 1's request keeps running in the background and lands in its own tab.
  await expect(page.getByTestId('tab-title').nth(0)).toHaveText('Mock: https://slow.com')

  await page.getByTestId('tab').nth(0).click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://slow.com')
  await expect(status).toHaveText('Loaded slow.com')

  // Tab 2 is untouched by tab 1's completion.
  await page.getByTestId('tab').nth(1).click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.com')
})

test('back alone during load discards the in-flight result', async ({ page }) => {
  const frame = page.frameLocator(frameSel)
  const back = page.getByTestId('back-button')
  const forward = page.getByTestId('forward-button')
  const status = page.getByTestId('status')

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')

  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://second.com')

  await page.getByTestId('address-bar').fill('slow.com')
  await page.getByTestId('go-button').click()
  await expect(status).toHaveText('Loading slow.com…')

  await back.click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(forward).toBeEnabled()

  // The cancelled result never lands: still on first, forward entry intact,
  // no error status from the aborted request.
  await page.waitForTimeout(SLOW_DELAY_MS)
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await expect(page.getByTestId('address-bar')).toHaveValue('first.com')
  await expect(status).toHaveText('Loaded first.com')
  await expect(forward).toBeEnabled()
})
