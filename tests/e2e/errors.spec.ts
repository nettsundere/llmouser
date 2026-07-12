import { test, expect } from './fixtures'

test('shows an error state when generation fails', async ({ page }) => {
  // The mock provider rejects for any URL containing "throw-error".
  await page.getByTestId('address-bar').fill('throw-error.test')
  await page.getByTestId('go-button').click()

  const status = page.getByTestId('status')
  await expect(status).toHaveClass(/error/)
  await expect(status).toContainText('Failed to load')
  await expect(status).toContainText('Mock provider forced error')
})
