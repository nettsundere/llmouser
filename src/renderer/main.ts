import './env.d'
import { normalizeUrl } from '@shared/url'
import { injectBase } from '@shared/site-html'
import { initSettingsUi } from './settings-ui'

const HISTORY_LIMIT = 10

interface Tab {
  id: number
  title: string
  /** SVG favicon as a data URI, extracted from the generated page. */
  icon: string | null
  /** Text shown in the address bar for this tab. */
  addressValue: string
  /** Absolute URL of the page currently shown; base for relative links. */
  currentUrl: string
  /** Visited URLs, oldest first — session context for the LLM. */
  history: string[]
  /** Raw generated HTML of the current page — source for Save as PDF. */
  html: string
  statusMessage: string
  statusError: boolean
  frame: HTMLIFrameElement
}

function el<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id)
  if (!node) throw new Error(`Missing element #${id}`)
  return node as T
}

const form = el<HTMLFormElement>('address-form')
const address = el<HTMLInputElement>('address-bar')
const status = el<HTMLElement>('status')
const viewport = el<HTMLElement>('viewport')
const tabBar = el<HTMLElement>('tab-bar')
const newTabButton = el<HTMLButtonElement>('new-tab-button')

const tabs: Tab[] = []
let active: Tab | null = null
let nextTabId = 1

function setStatus(message: string, isError = false): void {
  status.textContent = message
  status.classList.toggle('error', isError)
}

function setTabStatus(tab: Tab, message: string, isError = false): void {
  tab.statusMessage = message
  tab.statusError = isError
  if (tab === active) setStatus(message, isError)
}

/**
 * The site frame is a srcdoc iframe, so it has no real base URL — a plain link click
 * would resolve relative hrefs against the app's own index.html and load the browser
 * chrome inside the frame. This script intercepts link clicks and form submits inside
 * the generated page and forwards the target to the chrome via postMessage; the chrome
 * then generates that URL like any other navigation.
 */
const NAV_INTERCEPTOR = `<script>(function () {
  function send(href) {
    parent.postMessage({ type: 'llm-navigate', href: href }, '*')
  }
  document.addEventListener('click', function (event) {
    var target = event.target
    var anchor = target && target.closest ? target.closest('a[href]') : null
    if (!anchor) return
    var href = anchor.getAttribute('href') || ''
    if (href.charAt(0) === '#') {
      // In-page anchor: with an injected <base> this would become a real
      // cross-document navigation, so scroll manually instead.
      event.preventDefault()
      var id = href.slice(1)
      var section = document.getElementById(id) || document.getElementsByName(id)[0]
      if (section && section.scrollIntoView) section.scrollIntoView()
      return
    }
    event.preventDefault()
    if (href.slice(0, 11).toLowerCase() === 'javascript:') return
    send(href)
  }, true)
  document.addEventListener('submit', function (event) {
    var form = event.target
    if (!form || form.tagName !== 'FORM') return
    event.preventDefault()
    var action = form.getAttribute('action') || ''
    var params = new URLSearchParams(new FormData(form)).toString()
    var href = action + (params ? (action.indexOf('?') >= 0 ? '&' : '?') + params : '')
    send(href)
  }, true)
})()<\/script>`

function injectInterceptor(html: string): string {
  if (/<\/body>/i.test(html)) {
    return html.replace(/<\/body>/i, `${NAV_INTERCEPTOR}</body>`)
  }
  return html + NAV_INTERCEPTOR
}

function extractTitle(html: string): string {
  const match = /<title[^>]*>([^<]*)<\/title>/i.exec(html)
  return match ? match[1].trim() : ''
}

/** Pull the LLM-generated SVG favicon (data URI) out of the page's <head>. */
function extractIcon(html: string): string | null {
  const linkTag = /<link\b[^>]*\brel\s*=\s*["']?[^"'>]*icon[^"'>]*["']?[^>]*>/i.exec(html)
  if (!linkTag) return null
  const href = /href\s*=\s*(?:"([^"]*)"|'([^']*)')/i.exec(linkTag[0])
  const value = href ? (href[1] ?? href[2]) : null
  return value && value.startsWith('data:image/svg+xml') ? value : null
}

/** Fallback tab icon when the generated page has no usable favicon. */
function letterIcon(url: string): string {
  let host = url
  try {
    host = new URL(url).host
  } catch {
    // Not a URL: use the raw string.
  }
  const cleaned = host.replace(/^www\./, '')
  const first = cleaned.charAt(0).toUpperCase()
  const letter = /[A-Z0-9]/.test(first) ? first : '?'
  let hash = 0
  for (let i = 0; i < cleaned.length; i++) {
    hash = (hash * 31 + cleaned.charCodeAt(i)) | 0
  }
  const hue = Math.abs(hash) % 360
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">' +
    `<rect width="16" height="16" rx="3" fill="hsl(${hue},60%,45%)"/>` +
    '<text x="8" y="12" font-family="system-ui,sans-serif" font-size="10" ' +
    `font-weight="bold" fill="#fff" text-anchor="middle">${letter}</text></svg>`
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`
}

const NEW_TAB_ICON =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">' +
      '<rect width="16" height="16" rx="3" fill="#a1a1aa"/></svg>'
  )

function renderTabBar(): void {
  tabBar.querySelectorAll('.tab').forEach((node) => node.remove())
  for (const tab of tabs) {
    const node = document.createElement('div')
    node.className = 'tab' + (tab === active ? ' active' : '')
    node.setAttribute('data-testid', 'tab')

    const icon = document.createElement('img')
    icon.className = 'tab-icon'
    icon.setAttribute('data-testid', 'tab-icon')
    icon.alt = ''
    icon.src = tab.icon ?? (tab.currentUrl ? letterIcon(tab.currentUrl) : NEW_TAB_ICON)

    const title = document.createElement('span')
    title.className = 'tab-title'
    title.setAttribute('data-testid', 'tab-title')
    title.textContent = tab.title

    const close = document.createElement('button')
    close.className = 'tab-close'
    close.setAttribute('data-testid', 'tab-close')
    close.type = 'button'
    close.title = 'Close tab'
    close.textContent = '×'
    close.addEventListener('click', (event) => {
      event.stopPropagation()
      closeTab(tab)
    })

    node.append(icon, title, close)
    node.addEventListener('click', () => activateTab(tab))
    tabBar.insertBefore(node, newTabButton)
  }
}

function activateTab(tab: Tab): void {
  active = tab
  for (const t of tabs) {
    const isActive = t === tab
    t.frame.hidden = !isActive
    // Exactly one iframe carries the site-frame testid: the visible one.
    if (isActive) t.frame.setAttribute('data-testid', 'site-frame')
    else t.frame.removeAttribute('data-testid')
  }
  address.value = tab.addressValue
  setStatus(tab.statusMessage, tab.statusError)
  renderTabBar()
  if (!tab.currentUrl) address.focus()
}

function createTab(): Tab {
  const frame = document.createElement('iframe')
  frame.className = 'site-frame'
  frame.setAttribute('sandbox', 'allow-scripts allow-forms')
  frame.title = 'Rendered site'
  frame.hidden = true
  viewport.appendChild(frame)

  const tab: Tab = {
    id: nextTabId++,
    title: 'New tab',
    icon: null,
    addressValue: '',
    currentUrl: '',
    history: [],
    html: '',
    statusMessage: 'Ready',
    statusError: false,
    frame
  }
  tabs.push(tab)
  activateTab(tab)
  return tab
}

function closeTab(tab: Tab): void {
  const index = tabs.indexOf(tab)
  if (index === -1) return
  tabs.splice(index, 1)
  tab.frame.remove()
  if (tabs.length === 0) {
    createTab()
    return
  }
  if (active === tab) {
    activateTab(tabs[Math.min(index, tabs.length - 1)])
  } else {
    renderTabBar()
  }
}

async function navigate(tab: Tab, url: string): Promise<void> {
  const input = url.trim()
  if (!input) return

  tab.addressValue = input
  if (tab === active) address.value = input
  setTabStatus(tab, `Loading ${input}…`)
  try {
    const html = await window.llmBrowser.navigate(input, {
      referer: tab.history[tab.history.length - 1],
      history: [...tab.history]
    })
    tab.currentUrl = normalizeUrl(input)
    tab.history.push(tab.currentUrl)
    if (tab.history.length > HISTORY_LIMIT) tab.history.shift()
    tab.title = extractTitle(html) || tab.currentUrl
    tab.icon = extractIcon(html) ?? letterIcon(tab.currentUrl)
    tab.html = html
    tab.frame.srcdoc = injectBase(injectInterceptor(html), tab.currentUrl)
    setTabStatus(tab, `Loaded ${input}`)
  } catch (error) {
    tab.frame.srcdoc = ''
    tab.html = ''
    const message = error instanceof Error ? error.message : String(error)
    setTabStatus(tab, `Failed to load ${input}: ${message}`, true)
  }
  renderTabBar()
}

function navigateTo(tab: Tab, target: URL): void {
  if (target.protocol !== 'http:' && target.protocol !== 'https:') return
  // Hash-only change on the current page: in-page scroll, not a new document.
  if (
    target.hash &&
    target.toString().replace(/#.*$/, '') === tab.currentUrl.replace(/#.*$/, '')
  ) {
    return
  }
  void navigate(tab, target.toString())
}

form.addEventListener('submit', (event) => {
  event.preventDefault()
  if (active) void navigate(active, address.value)
})

newTabButton.addEventListener('click', () => createTab())

async function savePagePdf(tab: Tab): Promise<void> {
  setTabStatus(tab, 'Saving PDF…')
  try {
    const result = await window.llmBrowser.savePdf(tab.currentUrl, tab.html)
    if (result.saved) {
      setTabStatus(tab, `Saved PDF: ${result.path}`)
    } else {
      setTabStatus(tab, 'PDF save canceled')
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    setTabStatus(tab, `Failed to save PDF: ${message}`, true)
  }
}

// File > Save Page as PDF in the app menu.
window.llmBrowser.onSavePdfRequest(() => {
  const tab = active
  if (!tab) return
  if (!tab.html) {
    setTabStatus(tab, 'Nothing to save as PDF')
    return
  }
  void savePagePdf(tab)
})

// Link clicks / form submits inside a generated page arrive here.
window.addEventListener('message', (event) => {
  const tab = tabs.find((t) => t.frame.contentWindow === event.source)
  if (!tab) return
  const data = event.data as { type?: string; href?: string } | null
  if (!data || data.type !== 'llm-navigate' || typeof data.href !== 'string') return

  try {
    navigateTo(tab, new URL(data.href, tab.currentUrl || undefined))
  } catch {
    // Unresolvable href: ignore.
  }
})

/**
 * A real navigation the main process blocked (location.href, meta refresh, a link
 * the injected script missed). Attribute it to the active tab — that's where user
 * interaction happens. If the srcdoc frame resolved a relative target against the
 * chrome's own base, recover the intended path against the tab's current URL.
 */
window.llmBrowser.onBlockedNavigation((rawUrl) => {
  const tab = active
  if (!tab) return

  let url: URL
  try {
    url = new URL(rawUrl)
  } catch {
    return
  }

  const own = window.location
  const resolvedAgainstChrome =
    url.protocol === 'file:' || (url.protocol === own.protocol && url.host === own.host)

  try {
    if (resolvedAgainstChrome) {
      if (!tab.currentUrl) return
      navigateTo(tab, new URL(url.pathname + url.search + url.hash, tab.currentUrl))
    } else {
      navigateTo(tab, url)
    }
  } catch {
    // Unresolvable target: ignore.
  }
})

initSettingsUi()
createTab()
