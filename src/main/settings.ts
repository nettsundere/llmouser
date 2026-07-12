import Store from 'electron-store'
import type { LlmSettings, ProviderName, PublicSettings, SettingsUpdate } from '@shared/types'
import { DEFAULT_UNIVERSE } from '@shared/types'

const DEFAULT_ENDPOINTS: Record<ProviderName, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com'
}

const DEFAULT_MODELS: Record<ProviderName, string> = {
  openai: 'gpt-4o',
  anthropic: 'claude-sonnet-5'
}

const defaults: LlmSettings = {
  provider: 'openai',
  endpoint: DEFAULT_ENDPOINTS.openai,
  apiKey: '',
  model: DEFAULT_MODELS.openai,
  universe: DEFAULT_UNIVERSE
}

// Lazy: constructing the store resolves the userData path, which E2E overrides via
// app.setPath() in index.ts. Import hoisting would run an eager constructor first.
let store: Store<LlmSettings> | undefined

function getStore(): Store<LlmSettings> {
  store ??= new Store<LlmSettings>({ name: 'llm-browser-settings', defaults })
  return store
}

/** Full settings, including the API key. Main-process use only. */
export function getSettings(): LlmSettings {
  const s = getStore()
  return {
    provider: s.get('provider'),
    endpoint: s.get('endpoint'),
    apiKey: s.get('apiKey'),
    model: s.get('model'),
    universe: s.get('universe') || DEFAULT_UNIVERSE
  }
}

/** Renderer-safe view: the API key itself never crosses the IPC boundary. */
export function getPublicSettings(): PublicSettings {
  const settings = getSettings()
  return {
    provider: settings.provider,
    endpoint: settings.endpoint,
    model: settings.model,
    hasApiKey: settings.apiKey.length > 0,
    universe: settings.universe
  }
}

export function saveSettings(update: SettingsUpdate): PublicSettings {
  const s = getStore()
  const provider: ProviderName = update.provider === 'anthropic' ? 'anthropic' : 'openai'
  s.set('provider', provider)
  s.set('endpoint', update.endpoint?.trim() || DEFAULT_ENDPOINTS[provider])
  s.set('model', update.model?.trim() || DEFAULT_MODELS[provider])
  s.set('universe', update.universe?.trim() || DEFAULT_UNIVERSE)
  // Empty key field means "keep the stored key" — the UI never shows the saved value.
  if (update.apiKey && update.apiKey.trim().length > 0) {
    s.set('apiKey', update.apiKey.trim())
  }
  return getPublicSettings()
}

export { DEFAULT_ENDPOINTS, DEFAULT_MODELS }
