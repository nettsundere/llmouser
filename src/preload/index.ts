import { contextBridge, ipcRenderer } from 'electron'
import type { LlmBrowserApi, NavigateContext, SettingsUpdate } from '@shared/types'

const api: LlmBrowserApi = {
  navigate: (url: string, context?: NavigateContext) =>
    ipcRenderer.invoke('navigate', url, context),
  getSettings: () => ipcRenderer.invoke('settings:get'),
  saveSettings: (settings: SettingsUpdate) => ipcRenderer.invoke('settings:save', settings),
  onBlockedNavigation: (callback: (url: string) => void) => {
    ipcRenderer.on('frame-navigate-blocked', (_event, url: string) => callback(url))
  }
}

contextBridge.exposeInMainWorld('llmBrowser', api)
