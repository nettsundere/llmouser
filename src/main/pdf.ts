import { unlink, writeFile } from 'fs/promises'
import { join } from 'path'
import { app, BrowserWindow, dialog } from 'electron'
import { injectBase } from '@shared/site-html'
import type { SavePdfResult } from '@shared/types'

/** Turn the page URL into a friendly default file name. */
function defaultPdfName(url: string): string {
  let base = url
  try {
    const parsed = new URL(url)
    base = parsed.host + parsed.pathname
  } catch {
    // Not a URL: use the raw string.
  }
  const cleaned = base
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return `${cleaned || 'page'}.pdf`
}

/**
 * Render the generated page in a hidden window and print it to PDF. The HTML
 * comes from the renderer (only tabs hold page content); it is re-wrapped with
 * the same CSP + <base> the visible frame gets, so the hidden copy can't reach
 * the network or navigate anywhere real either.
 */
export async function savePageAsPdf(
  owner: BrowserWindow | null,
  url: string,
  html: string
): Promise<SavePdfResult> {
  const dialogOptions = {
    title: 'Save page as PDF',
    defaultPath: defaultPdfName(url),
    filters: [{ name: 'PDF', extensions: ['pdf'] }]
  }
  const { canceled, filePath } = owner
    ? await dialog.showSaveDialog(owner, dialogOptions)
    : await dialog.showSaveDialog(dialogOptions)
  if (canceled || !filePath) return { saved: false }

  // printToPDF needs a real loaded document, so stage the HTML in a temp file.
  const tempPath = join(app.getPath('temp'), `llmouser-pdf-${process.pid}-${Date.now()}.html`)
  await writeFile(tempPath, injectBase(html, url), 'utf8')

  const win = new BrowserWindow({
    show: false,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false }
  })
  win.webContents.on('will-navigate', (event) => event.preventDefault())
  try {
    await win.loadFile(tempPath)
    const pdf = await win.webContents.printToPDF({ printBackground: true })
    await writeFile(filePath, pdf)
    return { saved: true, path: filePath }
  } finally {
    win.destroy()
    await unlink(tempPath).catch(() => {})
  }
}
