//! macOS shell: AppKit windows with native window tabs, a unified toolbar and
//! WKWebView, driven through `objc2`.

mod about;
mod app;
mod automation;
mod menu;
mod pdf;
mod settings;
mod util;
mod webview;
mod window;

pub fn run() {
    app::run();
}
