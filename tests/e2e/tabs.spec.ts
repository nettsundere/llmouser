import { test, expect } from './fixtures'

// The active tab's iframe carries the site-frame testid.
const frameSel = '[data-testid="site-frame"]'

test('tabs open independent sites with generated icons and titles', async ({ page }) => {
  const frame = page.frameLocator(frameSel)

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')

  await page.getByTestId('new-tab-button').click()
  await expect(page.getByTestId('tab')).toHaveCount(2)
  await expect(page.getByTestId('address-bar')).toHaveValue('')

  await page.getByTestId('address-bar').fill('other.site')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.site')

  const tabItems = page.getByTestId('tab')
  await expect(tabItems.nth(0).getByTestId('tab-title')).toHaveText('Mock: https://example.com')
  await expect(tabItems.nth(1).getByTestId('tab-title')).toHaveText('Mock: https://other.site')

  // Icons come from the SVG favicon generated with each site.
  await expect(tabItems.nth(0).getByTestId('tab-icon')).toHaveAttribute(
    'src',
    /^data:image\/svg\+xml/
  )
  await expect(tabItems.nth(1).getByTestId('tab-icon')).toHaveAttribute(
    'src',
    /^data:image\/svg\+xml/
  )

  // Switching back restores that tab's page and address.
  await tabItems.nth(0).click()
  await expect(page.getByTestId('address-bar')).toHaveValue('example.com')
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
})

test('tabs have independent browsing history / referer context', async ({ page }) => {
  const frame = page.frameLocator(frameSel)

  await page.getByTestId('address-bar').fill('first.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://first.com')
  await page.getByTestId('address-bar').fill('second.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-referer')).toHaveText('https://first.com')

  // A fresh tab starts a fresh session: no referer bleed from the other tab.
  await page.getByTestId('new-tab-button').click()
  await page.getByTestId('address-bar').fill('third.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://third.com')
  await expect(frame.getByTestId('mock-referer')).toHaveText('none')
})

test('closing tabs activates a neighbor; last close leaves a fresh tab', async ({ page }) => {
  const frame = page.frameLocator(frameSel)

  await page.getByTestId('address-bar').fill('example.com')
  await page.getByTestId('go-button').click()
  await page.getByTestId('new-tab-button').click()
  await page.getByTestId('address-bar').fill('other.site')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.site')

  // Close the active (second) tab: first tab's page comes back.
  await page.getByTestId('tab').nth(1).getByTestId('tab-close').click()
  await expect(page.getByTestId('tab')).toHaveCount(1)
  await expect(frame.getByTestId('mock-url')).toHaveText('https://example.com')
  await expect(page.getByTestId('address-bar')).toHaveValue('example.com')

  // Closing the last tab leaves a single fresh one.
  await page.getByTestId('tab').getByTestId('tab-close').click()
  await expect(page.getByTestId('tab')).toHaveCount(1)
  await expect(page.getByTestId('tab').getByTestId('tab-title')).toHaveText('New tab')
  await expect(page.getByTestId('address-bar')).toHaveValue('')
})

test('falls back to a generated letter icon when the page has no favicon', async ({ page }) => {
  await page.getByTestId('address-bar').fill('noicon.test')
  await page.getByTestId('go-button').click()

  const icon = page.getByTestId('tab').getByTestId('tab-icon')
  // The procedural fallback is an SVG data URI containing a <text> letter glyph.
  await expect(icon).toHaveAttribute('src', /^data:image\/svg\+xml/)
  await expect(icon).toHaveAttribute('src', /%3Ctext/)
})
