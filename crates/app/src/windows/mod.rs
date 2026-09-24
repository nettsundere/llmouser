//! Windows shell: Win32 windows, menus and common controls with WebView2.

mod about;
mod app;
mod automation;
mod menu;
mod settings;
mod util;
mod webview;
mod window;

pub fn run() {
    app::run();
}
