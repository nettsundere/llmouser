import type { LlmSettings } from '@shared/types'
import { Provider, SiteRequest } from './provider'

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

/**
 * Deterministic offline provider used by E2E tests (LLM_BROWSER_MOCK=1).
 * Echoes the URL, query params and referer, and includes a search form,
 * navigation links and a JS-navigation button to exercise every nav path.
 * A URL containing "throw-error" makes it reject, exercising the error path.
 */
export const mockProvider: Provider = {
  async generateSite(request: SiteRequest, settings: LlmSettings): Promise<string> {
    if (request.url.includes('throw-error')) {
      throw new Error('Mock provider forced error')
    }
    const safeUrl = escapeHtml(request.url)

    let params = ''
    let favicon = ''
    try {
      const url = new URL(request.url)
      params = [...url.searchParams.entries()]
        .map(([k, v]) => `<li data-param="${escapeHtml(k)}">${escapeHtml(k)}=${escapeHtml(v)}</li>`)
        .join('')
      // "noicon" hosts omit the favicon so tests can exercise the fallback icon.
      if (!url.host.includes('noicon')) {
        favicon =
          '<link rel="icon" href="data:image/svg+xml,' +
          '%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20viewBox=%220%200%2016%2016%22%3E' +
          '%3Crect%20width=%2216%22%20height=%2216%22%20rx=%223%22%20fill=%22%234a90d9%22/%3E' +
          '%3C/svg%3E">'
      }
    } catch {
      // Unparseable URL: no params or favicon to show.
    }

    return `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Mock: ${safeUrl}</title>${favicon}</head>
<body>
  <h1 id="mock-heading">Mock page for ${safeUrl}</h1>
  <p data-testid="mock-url">${safeUrl}</p>
  <p data-testid="mock-provider">${escapeHtml(settings.provider)}</p>
  <p data-testid="mock-referer">${escapeHtml(request.referer ?? 'none')}</p>
  <p data-testid="mock-universe">${escapeHtml(settings.universe)}</p>
  <ul data-testid="mock-params">${params}</ul>
  <form id="mock-search-form" action="/search">
    <input id="mock-search-input" name="q" type="text" />
    <button id="mock-search-submit" type="submit">Search</button>
  </form>
  <nav>
    <a id="mock-link-relative" href="/about">About</a>
    <a id="mock-link-absolute" href="https://other.example/page">Other site</a>
    <a id="mock-link-hash" href="#section">Jump</a>
  </nav>
  <button id="mock-js-nav" onclick="location.href='/js-nav'">JS nav</button>
  <img id="mock-ext-img" src="https://cdn.example/logo.png" alt=""
    onload="this.setAttribute('data-net','loaded')"
    onerror="this.setAttribute('data-net','blocked')" />
  <div id="mock-fetch-result">pending</div>
  <script>
    fetch('https://api.example/data')
      .then(function () { document.getElementById('mock-fetch-result').textContent = 'fetch-ok' })
      .catch(function () { document.getElementById('mock-fetch-result').textContent = 'fetch-blocked' })
  </script>
</body>
</html>`
  }
}
