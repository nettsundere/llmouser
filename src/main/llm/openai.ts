import type { LlmSettings } from '@shared/types'
import { Provider, SiteRequest, systemPrompt, userPrompt, stripFences } from './provider'

interface OpenAiResponse {
  choices?: Array<{ message?: { content?: string } }>
}

export const openaiProvider: Provider = {
  async generateSite(request: SiteRequest, settings: LlmSettings): Promise<string> {
    const res = await fetch(`${settings.endpoint}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${settings.apiKey}`
      },
      body: JSON.stringify({
        model: settings.model,
        max_tokens: settings.maxTokens,
        messages: [
          { role: 'system', content: systemPrompt(settings.universe) },
          { role: 'user', content: userPrompt(request) }
        ]
      })
    })

    if (!res.ok) {
      throw new Error(`OpenAI request failed (${res.status}): ${await res.text()}`)
    }

    const data = (await res.json()) as OpenAiResponse
    const content = data.choices?.[0]?.message?.content
    if (!content) {
      throw new Error('OpenAI response contained no content')
    }
    return stripFences(content)
  }
}
