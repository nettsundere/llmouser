import type { Language } from '@shared/types'
import { DEFAULT_LANGUAGE } from '@shared/types'
import { MESSAGES, type Messages } from '@shared/i18n'

let current: Language = DEFAULT_LANGUAGE
const listeners = new Set<() => void>()

export function messages(): Messages {
  return MESSAGES[current]
}

export function currentLanguage(): Language {
  return current
}

/** Switch the chrome language: retranslate static markup and notify subscribers. */
export function setLanguage(language: Language): void {
  if (!MESSAGES[language] || language === current) return
  current = language
  applyStaticTranslations()
  listeners.forEach((listener) => listener())
}

/** Runs on every language change, after the static DOM has been retranslated. */
export function onLanguageChange(listener: () => void): void {
  listeners.add(listener)
}

/** String-valued message for elements translated via data-i18n* attributes. */
function text(key: string): string {
  const value = messages()[key as keyof Messages]
  return typeof value === 'string' ? value : key
}

/**
 * Translate everything declared in the markup:
 *   data-i18n             -> textContent
 *   data-i18n-title       -> title (and aria-label when present)
 *   data-i18n-placeholder -> placeholder
 *   data-i18n-aria        -> aria-label
 */
export function applyStaticTranslations(): void {
  document.documentElement.lang = current
  document.querySelectorAll<HTMLElement>('[data-i18n]').forEach((node) => {
    node.textContent = text(node.dataset.i18n!)
  })
  document.querySelectorAll<HTMLElement>('[data-i18n-title]').forEach((node) => {
    const value = text(node.dataset.i18nTitle!)
    node.title = value
    if (node.hasAttribute('aria-label')) node.setAttribute('aria-label', value)
  })
  document.querySelectorAll<HTMLInputElement>('[data-i18n-placeholder]').forEach((node) => {
    node.placeholder = text(node.dataset.i18nPlaceholder!)
  })
  document.querySelectorAll<HTMLElement>('[data-i18n-aria]').forEach((node) => {
    node.setAttribute('aria-label', text(node.dataset.i18nAria!))
  })
}
