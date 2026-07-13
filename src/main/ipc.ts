import { BrowserWindow, ipcMain } from 'electron'
import type { NavigateContext, PublicSettings, SavePdfResult, SettingsUpdate } from '@shared/types'
import { getPublicSettings, getSettings, saveSettings } from './settings'
import { getProvider, normalizeUrl } from './llm'
import type { SiteRequest } from './llm/provider'
import { savePageAsPdf } from './pdf'

const HISTORY_LIMIT = 10

export function registerIpc(): void {
  ipcMain.handle(
    'navigate',
    async (_event, url: string, context?: NavigateContext): Promise<string> => {
      const settings = getSettings()
      const provider = getProvider(settings)

      // History lives per-tab in the renderer; sanitize what it sends.
      const history = Array.isArray(context?.history)
        ? context.history.filter((u): u is string => typeof u === 'string').slice(-HISTORY_LIMIT)
        : []
      const referer =
        typeof context?.referer === 'string' && context.referer ? context.referer : undefined

      const request: SiteRequest = { url: normalizeUrl(url), referer, history }
      return provider.generateSite(request, settings)
    }
  )

  ipcMain.handle(
    'page:save-pdf',
    (event, url: string, html: string): Promise<SavePdfResult> => {
      if (typeof url !== 'string' || typeof html !== 'string' || !html) {
        throw new Error('Nothing to save')
      }
      return savePageAsPdf(BrowserWindow.fromWebContents(event.sender), url, html)
    }
  )

  ipcMain.handle('settings:get', (): PublicSettings => getPublicSettings())

  ipcMain.handle('settings:save', (_event, update: SettingsUpdate): PublicSettings =>
    saveSettings(update)
  )
}
