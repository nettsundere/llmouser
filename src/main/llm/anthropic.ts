import type { LlmSettings } from '@shared/types'
import { Provider, SiteRequest, systemPrompt, userPrompt, stripFences } from './provider'

interface AnthropicResponse {
  content?: Array<{ type: string; text?: string }>
}

export const anthropicProvider: Provider = {
  async generateSite(request: SiteRequest, settings: LlmSettings): Promise<string> {
    const res = await fetch(`${settings.endpoint}/v1/messages`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': settings.apiKey,
        'anthropic-version': '2023-06-01'
      },
      body: JSON.stringify({
        model: settings.model,
        max_tokens: 4096,
        system: systemPrompt(settings.universe),
        messages: [{ role: 'user', content: userPrompt(request) }]
      })
    })

    if (!res.ok) {
      throw new Error(`Anthropic request failed (${res.status}): ${await res.text()}`)
    }

    const data = (await res.json()) as AnthropicResponse
    const content = data.content?.find((b) => b.type === 'text')?.text
    if (!content) {
      throw new Error('Anthropic response contained no text content')
    }
    return stripFences(content)
  }
}
