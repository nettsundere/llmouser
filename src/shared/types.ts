export type ProviderName = 'openai' | 'anthropic'

/** The universe websites are generated in, unless the user redefines it. */
export const DEFAULT_UNIVERSE =
  'Our real universe, exactly as it is today: real sites, brands, people and events.'

/** Safe across providers (e.g. Anthropic rejects max_tokens above the model's limit). */
export const DEFAULT_MAX_TOKENS = 64000

export interface LlmSettings {
  provider: ProviderName
  /** Base URL of the API, e.g. https://api.openai.com/v1 or https://api.anthropic.com */
  endpoint: string
  apiKey: string
  model: string
  /** Description of the universe all generated sites exist in. */
  universe: string
  /** Output token cap sent as max_tokens on every generation request. */
  maxTokens: number
}

/** Settings as exposed to the renderer — the API key itself never leaves main. */
export interface PublicSettings {
  provider: ProviderName
  endpoint: string
  model: string
  hasApiKey: boolean
  universe: string
  maxTokens: number
}

/** Settings update from the UI. Omitted/empty apiKey keeps the stored key. */
export interface SettingsUpdate {
  provider: ProviderName
  endpoint: string
  model: string
  apiKey?: string
  universe: string
  maxTokens: number
}

/** Per-tab session context sent along with a navigation. */
export interface NavigateContext {
  referer?: string
  history?: string[]
}

/** API surface exposed to the renderer via contextBridge (preload). */
export interface LlmBrowserApi {
  navigate(url: string, context?: NavigateContext): Promise<string>
  getSettings(): Promise<PublicSettings>
  saveSettings(settings: SettingsUpdate): Promise<PublicSettings>
  /** Fires when the main process blocks a real navigation inside the site frame. */
  onBlockedNavigation(callback: (url: string) => void): void
}
