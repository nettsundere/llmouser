import { app, BrowserWindow, Menu } from 'electron'
import type { Language } from '@shared/types'
import { MESSAGES } from '@shared/i18n'

/**
 * Application menu: standard roles plus File > Save Page as PDF. Only the
 * renderer knows the active tab's page HTML, so the menu item just pings it;
 * the renderer answers with a `page:save-pdf` invoke carrying the HTML.
 *
 * Rebuilt on every language change. Items keep their Electron role (behavior
 * and accelerators) but carry explicit labels — role defaults follow the OS
 * locale, not the in-app language setting.
 */
export function installMenu(language: Language): void {
  const t = MESSAGES[language]
  const isMac = process.platform === 'darwin'
  const appName = app.name

  const macAppMenu: Electron.MenuItemConstructorOptions = {
    label: appName,
    submenu: [
      { role: 'about', label: t.menuAbout(appName) },
      { type: 'separator' },
      { role: 'services', label: t.menuServices },
      { type: 'separator' },
      { role: 'hide', label: t.menuHide(appName) },
      { role: 'hideOthers', label: t.menuHideOthers },
      { role: 'unhide', label: t.menuShowAll },
      { type: 'separator' },
      { role: 'quit', label: t.menuQuit(appName) }
    ]
  }

  const template: Electron.MenuItemConstructorOptions[] = [
    ...(isMac ? [macAppMenu] : []),
    {
      label: t.menuFile,
      submenu: [
        {
          id: 'save-pdf',
          label: t.menuSavePdf,
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
        isMac
          ? { role: 'close', label: t.menuCloseWindow }
          : { role: 'quit', label: t.menuExit }
      ]
    },
    {
      label: t.menuEdit,
      submenu: [
        { role: 'undo', label: t.menuUndo },
        { role: 'redo', label: t.menuRedo },
        { type: 'separator' },
        { role: 'cut', label: t.menuCut },
        { role: 'copy', label: t.menuCopy },
        { role: 'paste', label: t.menuPaste },
        ...(isMac
          ? ([
              { role: 'pasteAndMatchStyle', label: t.menuPasteMatchStyle },
              { role: 'delete', label: t.menuDelete },
              { role: 'selectAll', label: t.menuSelectAll }
            ] as Electron.MenuItemConstructorOptions[])
          : ([
              { role: 'delete', label: t.menuDelete },
              { type: 'separator' },
              { role: 'selectAll', label: t.menuSelectAll }
            ] as Electron.MenuItemConstructorOptions[]))
      ]
    },
    {
      label: t.menuView,
      submenu: [
        { role: 'reload', label: t.menuReload },
        { role: 'forceReload', label: t.menuForceReload },
        { role: 'toggleDevTools', label: t.menuToggleDevTools },
        { type: 'separator' },
        { role: 'resetZoom', label: t.menuResetZoom },
        { role: 'zoomIn', label: t.menuZoomIn },
        { role: 'zoomOut', label: t.menuZoomOut },
        { type: 'separator' },
        { role: 'togglefullscreen', label: t.menuToggleFullscreen }
      ]
    },
    {
      label: t.menuWindow,
      role: 'windowMenu',
      submenu: [
        { role: 'minimize', label: t.menuMinimize },
        { role: 'zoom', label: t.menuZoomWindow },
        ...(isMac
          ? ([
              { type: 'separator' },
              { role: 'front', label: t.menuFront }
            ] as Electron.MenuItemConstructorOptions[])
          : ([{ role: 'close', label: t.menuCloseWindow }] as Electron.MenuItemConstructorOptions[]))
      ]
    }
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}
