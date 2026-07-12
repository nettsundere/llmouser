import type { LlmSettings } from '@shared/types'
import { Provider } from './provider'
import { openaiProvider } from './openai'
import { anthropicProvider } from './anthropic'
import { mockProvider } from './mock'

export { normalizeUrl } from '@shared/url'

/** Select the active provider from settings, or the mock in test mode. */
export function getProvider(settings: LlmSettings): Provider {
  if (process.env.LLM_BROWSER_MOCK === '1') {
    return mockProvider
  }
  return settings.provider === 'anthropic' ? anthropicProvider : openaiProvider
}
