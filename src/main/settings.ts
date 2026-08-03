import { app } from 'electron'
import Store from 'electron-store'
import type { Language, LlmSettings, ProviderName, PublicSettings, SettingsUpdate } from '@shared/types'
import { DEFAULT_MAX_TOKENS, DEFAULT_UNIVERSE, LANGUAGES } from '@shared/types'

const DEFAULT_ENDPOINTS: Record<ProviderName, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com'
}

const DEFAULT_MODELS: Record<ProviderName, string> = {
  openai: 'gpt-4o',
  anthropic: 'claude-sonnet-5'
}

/** First-run default: follow the OS locale; the settings selector overrides it. */
function detectLanguage(): Language {
  const locale = app.getLocale().toLowerCase()
  if (locale.startsWith('ru')) return 'ru'
  if (locale.startsWith('zh')) return 'zh'
  return 'en'
}

// Lazy: constructing the store resolves the userData path, which E2E overrides via
// app.setPath() in index.ts. Import hoisting would run an eager constructor first.
// (Lazy also matters for detectLanguage: app.getLocale() needs the app to be ready.)
let store: Store<LlmSettings> | undefined

function getStore(): Store<LlmSettings> {
  store ??= new Store<LlmSettings>({
    name: 'llm-browser-settings',
    defaults: {
      provider: 'openai',
      endpoint: DEFAULT_ENDPOINTS.openai,
      apiKey: '',
      model: DEFAULT_MODELS.openai,
      universe: DEFAULT_UNIVERSE,
      maxTokens: DEFAULT_MAX_TOKENS,
      language: detectLanguage()
    }
  })
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
    universe: s.get('universe') || DEFAULT_UNIVERSE,
    maxTokens: s.get('maxTokens') || DEFAULT_MAX_TOKENS,
    language: LANGUAGES.includes(s.get('language')) ? s.get('language') : detectLanguage()
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
    universe: settings.universe,
    maxTokens: settings.maxTokens,
    language: settings.language
  }
}

export function saveSettings(update: SettingsUpdate): PublicSettings {
  const s = getStore()
  const provider: ProviderName = update.provider === 'anthropic' ? 'anthropic' : 'openai'
  s.set('provider', provider)
  s.set('endpoint', update.endpoint?.trim() || DEFAULT_ENDPOINTS[provider])
  s.set('model', update.model?.trim() || DEFAULT_MODELS[provider])
  s.set('universe', update.universe?.trim() || DEFAULT_UNIVERSE)
  const maxTokens = Math.floor(Number(update.maxTokens))
  s.set('maxTokens', Number.isFinite(maxTokens) && maxTokens > 0 ? maxTokens : DEFAULT_MAX_TOKENS)
  if (LANGUAGES.includes(update.language)) {
    s.set('language', update.language)
  }
  // Empty key field means "keep the stored key" — the UI never shows the saved value.
  if (update.apiKey && update.apiKey.trim().length > 0) {
    s.set('apiKey', update.apiKey.trim())
  }
  return getPublicSettings()
}

export { DEFAULT_ENDPOINTS, DEFAULT_MODELS }
