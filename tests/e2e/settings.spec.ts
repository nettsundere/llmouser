import { test, expect } from './fixtures'

test('switching provider prefills endpoint and model defaults', async ({ page }) => {
  await page.getByTestId('settings-button').click()
  await expect(page.getByTestId('settings-panel')).toBeVisible()

  await page.getByTestId('settings-provider').selectOption('anthropic')
  await expect(page.getByTestId('settings-endpoint')).toHaveValue('https://api.anthropic.com')
  await expect(page.getByTestId('settings-model')).toHaveValue('claude-sonnet-5')

  await page.getByTestId('settings-provider').selectOption('openai')
  await expect(page.getByTestId('settings-endpoint')).toHaveValue('https://api.openai.com/v1')
  await expect(page.getByTestId('settings-model')).toHaveValue('gpt-4o')
})

test('saved settings persist across a renderer reload without exposing the key', async ({
  page
}) => {
  await page.getByTestId('settings-button').click()
  const apiKeyInput = page.getByTestId('settings-apikey')

  // Fresh install: no default secrets, empty key field.
  await expect(apiKeyInput).toHaveValue('')
  await expect(apiKeyInput).toHaveAttribute('placeholder', 'Enter API key')
  // Fresh install: max output tokens defaults to a provider-safe cap.
  await expect(page.getByTestId('settings-maxtokens')).toHaveValue('64000')

  await page.getByTestId('settings-provider').selectOption('anthropic')
  await page.getByTestId('settings-model').fill('claude-opus-4-8')
  await page.getByTestId('settings-maxtokens').fill('8192')
  await apiKeyInput.fill('secret-key-123')
  await page.getByTestId('settings-save').click()

  await expect(page.getByTestId('settings-panel')).toBeHidden()

  // Reload the renderer; settings are persisted in the main-process store.
  await page.reload()
  await page.getByTestId('settings-button').click()

  await expect(page.getByTestId('settings-provider')).toHaveValue('anthropic')
  await expect(page.getByTestId('settings-model')).toHaveValue('claude-opus-4-8')
  await expect(page.getByTestId('settings-maxtokens')).toHaveValue('8192')
  // The stored key is never sent back to the UI — only a "saved" hint.
  await expect(apiKeyInput).toHaveValue('')
  await expect(apiKeyInput).toHaveAttribute('placeholder', /Saved — leave blank to keep/)

  // Saving with a blank key field keeps the stored key.
  await page.getByTestId('settings-save').click()
  await page.getByTestId('settings-button').click()
  await expect(apiKeyInput).toHaveAttribute('placeholder', /Saved — leave blank to keep/)
})
