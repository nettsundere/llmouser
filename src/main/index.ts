import { join } from 'path'
import { app, BrowserWindow, nativeImage, session } from 'electron'
import { registerIpc } from './ipc'
import { installMenu } from './menu'

// Allow E2E tests to isolate settings storage per run. Must run before app is ready.
if (process.env.LLM_BROWSER_USERDATA) {
  app.setPath('userData', process.env.LLM_BROWSER_USERDATA)
}

// macOS gets the dock-style icon (inset squircle, transparent margin); Windows and
// Linux get the full-bleed, slightly rounded variant.
const appIcon = nativeImage.createFromPath(
  join(app.getAppPath(), process.platform === 'darwin' ? 'assets/icon-mac.png' : 'assets/icon.png')
)

function createWindow(): void {
  const win = new BrowserWindow({
    width: 1200,
    height: 800,
    title: 'LLMouser',
    icon: appIcon,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  })

  // The generated site lives in a srcdoc iframe with no real base URL, so any real
  // navigation it attempts (link the injected script missed, location.href, meta
  // refresh, nested <iframe src>) resolves against the chrome's own base and would
  // load the browser UI inside the frame. Block every subframe navigation at the
  // Electron layer and hand the URL back to the chrome to regenerate via the LLM.
  win.webContents.on('will-frame-navigate', (event) => {
    if (event.isMainFrame) return
    if (event.url.startsWith('about:')) return // srcdoc loads themselves
    event.preventDefault()
    // Only navigations of the site frame itself (direct child of the main frame)
    // trigger regeneration; deeper nested frames are just blocked.
    const isSiteFrame = event.frame?.parent != null && event.frame.parent.parent == null
    if (isSiteFrame) {
      win.webContents.send('frame-navigate-blocked', event.url)
    }
  })

  // target=_blank / window.open from generated content: never open real windows.
  win.webContents.setWindowOpenHandler(({ url }) => {
    win.webContents.send('frame-navigate-blocked', url)
    return { action: 'deny' }
  })

  if (process.env.ELECTRON_RENDERER_URL) {
    win.loadURL(process.env.ELECTRON_RENDERER_URL)
  } else {
    win.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

/**
 * No renderer — chrome or generated page — may reach the real network. Cancel every
 * http(s)/ws(s) request in the session; only local app resources (file:, data:,
 * blob:, about:, devtools) and, in dev, the Vite server pass. LLM API calls are
 * unaffected: they run in the main process via Node's fetch, outside this session.
 */
function lockDownNetwork(): void {
  const devOrigin = process.env.ELECTRON_RENDERER_URL
  session.defaultSession.webRequest.onBeforeRequest((details, callback) => {
    const url = details.url
    const allowed =
      url.startsWith('file://') ||
      url.startsWith('data:') ||
      url.startsWith('blob:') ||
      url.startsWith('about:') ||
      url.startsWith('devtools://') ||
      url.startsWith('chrome://') ||
      (devOrigin !== undefined &&
        (url.startsWith(devOrigin) || url.startsWith(devOrigin.replace(/^http/, 'ws'))))
    callback({ cancel: !allowed })
  })
}

app.whenReady().then(() => {
  // macOS ignores BrowserWindow.icon; the dock icon must be set explicitly
  // (packaged builds would get it from the bundle's .icns instead).
  if (!appIcon.isEmpty()) {
    app.dock?.setIcon(appIcon)
  }
  lockDownNetwork()
  registerIpc()
  installMenu()
  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})
