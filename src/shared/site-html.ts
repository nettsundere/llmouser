/**
 * In-document network lockdown for generated pages: no remote subresources, no
 * fetch/XHR/WebSocket. Inline scripts/styles and data: URIs stay usable, so the
 * pages remain interactive and can embed their own images/fonts. Second layer on
 * top of the main process's session-wide request blocking.
 */
export const SITE_CSP =
  "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; " +
  'img-src data:; font-src data:; media-src data:; frame-src about: data:'

/**
 * Give the document a base URL matching the virtual site. Relative links,
 * scripts setting location, form actions etc. then resolve to real https:// URLs
 * (never a local file:// base), so the main process reliably sees and blocks
 * them. The CSP meta rides along, ahead of any generated content.
 */
export function injectBase(html: string, baseUrl: string): string {
  const tag =
    `<meta http-equiv="Content-Security-Policy" content="${SITE_CSP}">` +
    `<base href="${baseUrl.replace(/"/g, '&quot;')}">`
  if (/<head[^>]*>/i.test(html)) {
    return html.replace(/<head[^>]*>/i, (match) => match + tag)
  }
  if (/<html[^>]*>/i.test(html)) {
    return html.replace(/<html[^>]*>/i, (match) => `${match}<head>${tag}</head>`)
  }
  return tag + html
}
