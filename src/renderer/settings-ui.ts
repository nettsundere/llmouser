import type { ProviderName, PublicSettings } from '@shared/types'

const DEFAULTS: Record<ProviderName, { endpoint: string; model: string }> = {
  openai: { endpoint: 'https://api.openai.com/v1', model: 'gpt-4o' },
  anthropic: { endpoint: 'https://api.anthropic.com', model: 'claude-sonnet-5' }
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id)
  if (!node) throw new Error(`Missing element #${id}`)
  return node as T
}

export function initSettingsUi(): void {
  const panel = el<HTMLElement>('settings-panel')
  const openButton = el<HTMLButtonElement>('settings-button')
  const closeButton = el<HTMLButtonElement>('settings-close')
  const form = el<HTMLFormElement>('settings-form')
  const provider = el<HTMLSelectElement>('settings-provider')
  const endpoint = el<HTMLInputElement>('settings-endpoint')
  const model = el<HTMLInputElement>('settings-model')
  const apiKey = el<HTMLInputElement>('settings-apikey')
  const universe = el<HTMLTextAreaElement>('settings-universe')
  const maxTokens = el<HTMLInputElement>('settings-maxtokens')

  function apply(settings: PublicSettings): void {
    provider.value = settings.provider
    endpoint.value = settings.endpoint
    model.value = settings.model
    universe.value = settings.universe
    maxTokens.value = String(settings.maxTokens)
    // The stored key is never shown; the field stays empty and blank means "keep".
    apiKey.value = ''
    apiKey.placeholder = settings.hasApiKey
      ? 'Saved — leave blank to keep, type to replace'
      : 'Enter API key'
  }

  // When the provider changes, prefill sensible endpoint/model defaults.
  provider.addEventListener('change', () => {
    const preset = DEFAULTS[provider.value as ProviderName]
    if (preset) {
      endpoint.value = preset.endpoint
      model.value = preset.model
    }
  })

  openButton.addEventListener('click', async () => {
    apply(await window.llmBrowser.getSettings())
    panel.hidden = false
  })

  closeButton.addEventListener('click', () => {
    panel.hidden = true
  })

  form.addEventListener('submit', async (event) => {
    event.preventDefault()
    const saved = await window.llmBrowser.saveSettings({
      provider: provider.value as ProviderName,
      endpoint: endpoint.value,
      model: model.value,
      apiKey: apiKey.value,
      universe: universe.value,
      maxTokens: Number(maxTokens.value)
    })
    apply(saved)
    panel.hidden = true
  })
}
