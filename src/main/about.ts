import { readFileSync } from 'fs'
import { app, BrowserWindow } from 'electron'
import type { Language } from '@shared/types'
import { MESSAGES } from '@shared/i18n'
import { appIconPath } from './icon'

export const ABOUT_COPYRIGHT = '© 2026 Vladimir Kiselev. MIT License'

let aboutWindow: BrowserWindow | null = null

/**
 * Custom About window. The native macOS about panel always shows the bundle
 * icon (setAboutPanelOptions' iconPath is Linux/Windows-only), and native
 * message boxes place the icon wherever the OS wants — a plain window is the
 * only way to center the logo and keep the layout identical everywhere.
 */
export function showAboutDialog(language: Language): void {
  if (aboutWindow && !aboutWindow.isDestroyed()) {
    aboutWindow.focus()
    return
  }

  const t = MESSAGES[language]
  const title = t.menuAbout('LLMouser')
  const icon = readFileSync(appIconPath).toString('base64')
  const html = `<!doctype html>
<html lang="${language}">
  <head>
    <meta charset="utf-8" />
    <title>${title}</title>
    <style>
      body {
        margin: 0;
        height: 100vh;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 6px;
        text-align: center;
        font-family: system-ui, sans-serif;
        background: #ffffff;
        color: #1d1d1f;
        user-select: none;
        -webkit-user-select: none;
      }
      img { width: 128px; height: 128px; }
      h1 { font-size: 16px; font-weight: 600; margin: 10px 0 0; }
      p { font-size: 12px; margin: 0; color: #6e6e73; }
      @media (prefers-color-scheme: dark) {
        body { background: #1e1e1e; color: #f2f2f2; }
        p { color: #a0a0a5; }
      }
    </style>
  </head>
  <body>
    <img src="data:image/png;base64,${icon}" alt="" />
    <h1>LLMouser</h1>
    <p>${t.aboutVersion} ${app.getVersion()}</p>
    <p>${ABOUT_COPYRIGHT}</p>
  </body>
</html>`

  aboutWindow = new BrowserWindow({
    width: 300,
    height: 330,
    useContentSize: true,
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    title,
    webPreferences: { sandbox: true, contextIsolation: true }
  })
  aboutWindow.setMenuBarVisibility(false)
  aboutWindow.on('closed', () => {
    aboutWindow = null
  })
  // Escape closes, like the native about panel.
  aboutWindow.webContents.on('before-input-event', (_event, input) => {
    if (input.key === 'Escape') aboutWindow?.close()
  })
  void aboutWindow.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`)
}
