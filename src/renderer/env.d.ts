import type { LlmBrowserApi } from '@shared/types'

declare global {
  interface Window {
    llmBrowser: LlmBrowserApi
  }
}

export {}
