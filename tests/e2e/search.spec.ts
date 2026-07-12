import { test, expect } from './fixtures'

test('search form works and session context chains across sites', async ({ page }) => {
  const frame = page.frameLocator('[data-testid="site-frame"]')

  // Visit "google.com" — first page of the session, no referer yet.
  await page.getByTestId('address-bar').fill('google.com')
  await page.getByTestId('go-button').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://google.com')
  await expect(frame.getByTestId('mock-referer')).toHaveText('none')

  // Search like on a real site: type a query, submit the in-page form.
  await frame.locator('#mock-search-input').fill('cat pictures')
  await frame.locator('#mock-search-submit').click()

  // Form action + query resolved against the site, regenerated with context.
  await expect(page.getByTestId('address-bar')).toHaveValue(
    'https://google.com/search?q=cat+pictures'
  )
  await expect(frame.getByTestId('mock-url')).toHaveText(
    'https://google.com/search?q=cat+pictures'
  )
  await expect(frame.locator('[data-param="q"]')).toHaveText('q=cat pictures')
  // The previous page traveled with the request as the Referer header.
  await expect(frame.getByTestId('mock-referer')).toHaveText('https://google.com')

  // Navigate on from the results to another site — context keeps chaining.
  await frame.locator('#mock-link-absolute').click()
  await expect(frame.getByTestId('mock-url')).toHaveText('https://other.example/page')
  await expect(frame.getByTestId('mock-referer')).toHaveText(
    'https://google.com/search?q=cat+pictures'
  )
})
