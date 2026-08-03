import type { Language, ProviderName, PublicSettings } from '@shared/types'
import { LANGUAGES } from '@shared/types'
import { MESSAGES } from '@shared/i18n'
import { messages, onLanguageChange, setLanguage } from './i18n'

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
  const language = el<HTMLSelectElement>('settings-language')
  const provider = el<HTMLSelectElement>('settings-provider')
  const endpoint = el<HTMLInputElement>('settings-endpoint')
  const model = el<HTMLInputElement>('settings-model')
  const apiKey = el<HTMLInputElement>('settings-apikey')
  const universe = el<HTMLTextAreaElement>('settings-universe')
  const maxTokens = el<HTMLInputElement>('settings-maxtokens')

  // Panel tabs: LLM settings and UX. Both live in the same form; switching
  // only toggles visibility, so unsaved edits survive a tab change.
  const tabs: Array<{ tab: HTMLButtonElement; section: HTMLElement }> = [
    { tab: el<HTMLButtonElement>('settings-tab-llm'), section: el<HTMLElement>('settings-section-llm') },
    { tab: el<HTMLButtonElement>('settings-tab-ux'), section: el<HTMLElement>('settings-section-ux') }
  ]

  function selectTab(selected: HTMLButtonElement): void {
    for (const { tab, section } of tabs) {
      const isActive = tab === selected
      tab.classList.toggle('active', isActive)
      tab.setAttribute('aria-selected', String(isActive))
      section.hidden = !isActive
    }
  }

  for (const { tab } of tabs) {
    tab.addEventListener('click', () => selectTab(tab))
  }

  // One option per language config, each labeled in its own language.
  for (const lang of LANGUAGES) {
    const option = document.createElement('option')
    option.value = lang
    option.textContent = MESSAGES[lang].languageName
    language.appendChild(option)
  }

  // Last settings received from main — base for the instant language save.
  let stored: PublicSettings | null = null

  function applyKeyPlaceholder(hasApiKey: boolean): void {
    // The stored key is never shown; the field stays empty and blank means "keep".
    apiKey.placeholder = hasApiKey ? messages().apiKeySaved : messages().apiKeyEnter
  }

  function apply(settings: PublicSettings): void {
    stored = settings
    language.value = settings.language
    provider.value = settings.provider
    endpoint.value = settings.endpoint
    model.value = settings.model
    universe.value = settings.universe
    maxTokens.value = String(settings.maxTokens)
    apiKey.value = ''
    applyKeyPlaceholder(settings.hasApiKey)
    setLanguage(settings.language)
  }

  onLanguageChange(() => {
    if (stored) applyKeyPlaceholder(stored.hasApiKey)
  })

  // When the provider changes, prefill sensible endpoint/model defaults.
  provider.addEventListener('change', () => {
    const preset = DEFAULTS[provider.value as ProviderName]
    if (preset) {
      endpoint.value = preset.endpoint
      model.value = preset.model
    }
  })

  // Language switches instantly: retranslate the whole chrome and persist right
  // away, using the stored settings so unsaved form edits are not committed.
  language.addEventListener('change', async () => {
    const lang = language.value as Language
    setLanguage(lang)
    if (!stored) return
    stored = await window.llmBrowser.saveSettings({
      provider: stored.provider,
      endpoint: stored.endpoint,
      model: stored.model,
      universe: stored.universe,
      maxTokens: stored.maxTokens,
      language: lang
    })
  })

  openButton.addEventListener('click', async () => {
    apply(await window.llmBrowser.getSettings())
    selectTab(tabs[0].tab)
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
      maxTokens: Number(maxTokens.value),
      language: language.value as Language
    })
    apply(saved)
    panel.hidden = true
  })

  // Startup: pick up the persisted language before the user opens the panel.
  void window.llmBrowser.getSettings().then((settings) => {
    stored = settings
    language.value = settings.language
    setLanguage(settings.language)
  })
}
