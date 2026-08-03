import { join } from 'path'
import { app, nativeImage } from 'electron'

// macOS gets the dock-style icon (inset squircle, transparent margin); Windows and
// Linux get the full-bleed, slightly rounded variant.
export const appIconPath = join(
  app.getAppPath(),
  process.platform === 'darwin' ? 'assets/icon-mac.png' : 'assets/icon.png'
)

export const appIcon = nativeImage.createFromPath(appIconPath)
