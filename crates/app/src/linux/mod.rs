//! Linux shell: GTK4 + libadwaita (header bar, tab bar) + WebKitGTK 6.0.
//! The gtk-rs bindings are safe Rust; this module needs no `unsafe`.

mod about;
mod app;
mod automation;
mod menu;
mod pdf;
mod settings;
mod webview;
mod window;

pub fn run() {
    app::run();
}
