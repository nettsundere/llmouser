import type { LlmSettings } from '@shared/types'

/** A simulated HTTP request: the target plus session context for coherent browsing. */
export interface SiteRequest {
  url: string
  /** URL of the page the user navigated from (previous visit). */
  referer?: string
  /** Recently visited URLs, oldest first. */
  history: string[]
}

export interface Provider {
  /** Generate a complete HTML document for the given request. */
  generateSite(request: SiteRequest, settings: LlmSettings): Promise<string>
}

export function systemPrompt(universe: string): string {
  return (
    'You are a web server simulating the entire internet. For each HTTP request you ' +
    'receive, respond with a single, complete, self-contained HTML document that the ' +
    'requested site would plausibly serve. Make pages functional: include working ' +
    'navigation links and forms (search boxes, etc). Honor the query string — a search ' +
    'results page must list plausible results for the query, each result linking to a ' +
    'real-looking URL on the relevant site. Use the Referer and X-Browsing-History ' +
    'headers to keep the session coherent: a page opened from search results should ' +
    'match the result that was clicked. In the <head>, include a favicon for the site ' +
    'as an inline SVG data URI: <link rel="icon" href="data:image/svg+xml,..."> with a ' +
    'small, simple, recognizable logo (URL-encode the SVG; use %22 or %27 for quotes). ' +
    'Respond with ONLY the raw HTML — no explanations, no markdown code fences.\n\n' +
    'THE UNIVERSE: the internet you simulate exists in the following universe, and ' +
    'every site, brand, person, event and fact must be consistent with it:\n' +
    universe
  )
}

/** Render the request as HTTP-style headers so session context travels with it. */
export function userPrompt(request: SiteRequest): string {
  const lines: string[] = []
  try {
    const url = new URL(request.url)
    lines.push(`GET ${url.pathname}${url.search} HTTP/1.1`, `Host: ${url.host}`)
  } catch {
    lines.push(`GET ${request.url} HTTP/1.1`)
  }
  if (request.referer) {
    lines.push(`Referer: ${request.referer}`)
  }
  if (request.history.length > 0) {
    lines.push(`X-Browsing-History: ${request.history.join(', ')}`)
  }
  return `Generate the webpage for this request:\n\n${lines.join('\n')}`
}

/** LLMs often wrap output in ```html fences despite instructions — strip them. */
export function stripFences(text: string): string {
  const trimmed = text.trim()
  const fence = /^```(?:html)?\s*\n([\s\S]*?)\n```$/i.exec(trimmed)
  return (fence ? fence[1] : trimmed).trim()
}
