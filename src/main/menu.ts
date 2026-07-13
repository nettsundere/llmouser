import { BrowserWindow, Menu } from 'electron'

/**
 * Application menu: standard roles plus File > Save Page as PDF. Only the
 * renderer knows the active tab's page HTML, so the menu item just pings it;
 * the renderer answers with a `page:save-pdf` invoke carrying the HTML.
 */
export function installMenu(): void {
  const isMac = process.platform === 'darwin'
  const template: Electron.MenuItemConstructorOptions[] = [
    ...(isMac ? ([{ role: 'appMenu' }] as Electron.MenuItemConstructorOptions[]) : []),
    {
      label: 'File',
      submenu: [
        {
          id: 'save-pdf',
          label: 'Save Page as PDF…',
          accelerator: 'CmdOrCtrl+S',
          click: (_item, window) => {
            const target =
              window instanceof BrowserWindow
                ? window
                : (BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0])
            target?.webContents.send('save-pdf-requested')
          }
        },
        { type: 'separator' },
        isMac ? { role: 'close' } : { role: 'quit' }
      ]
    },
    { role: 'editMenu' },
    { role: 'viewMenu' },
    { role: 'windowMenu' }
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}
