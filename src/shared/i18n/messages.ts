/** Every user-visible string of the browser chrome, in one language. */
export interface Messages {
  /** Name of the language, written in the language itself. */
  languageName: string

  // Toolbar
  back: string
  forward: string
  go: string
  settings: string
  addressBar: string
  addressPlaceholder: string
  newTab: string
  closeTab: string
  renderedSite: string

  // Settings panel
  settingsTitle: string
  settingsTabLlm: string
  settingsTabUx: string
  provider: string
  endpoint: string
  model: string
  apiKey: string
  apiKeyEnter: string
  apiKeySaved: string
  maxTokens: string
  universeRules: string
  universePlaceholder: string
  language: string
  save: string
  close: string

  // Status line
  ready: string
  loading(url: string): string
  loaded(url: string): string
  failedToLoad(url: string, error: string): string
  savingPdf: string
  savedPdf(path: string): string
  pdfCanceled: string
  failedPdf(error: string): string
  nothingToSave: string

  // Application menu. Role items keep their Electron behavior; these labels
  // override the OS-locale ones so the menu follows the in-app language.
  menuFile: string
  menuSavePdf: string
  menuCloseWindow: string
  menuQuit(appName: string): string
  menuExit: string

  menuAbout(appName: string): string
  /** "Version" label in the About dialog, prefixed to the version number. */
  aboutVersion: string
  menuServices: string
  menuHide(appName: string): string
  menuHideOthers: string
  menuShowAll: string

  menuEdit: string
  menuUndo: string
  menuRedo: string
  menuCut: string
  menuCopy: string
  menuPaste: string
  menuPasteMatchStyle: string
  menuDelete: string
  menuSelectAll: string

  menuView: string
  menuReload: string
  menuForceReload: string
  menuToggleDevTools: string
  menuResetZoom: string
  menuZoomIn: string
  menuZoomOut: string
  menuToggleFullscreen: string

  menuWindow: string
  menuMinimize: string
  menuZoomWindow: string
  menuFront: string
}
