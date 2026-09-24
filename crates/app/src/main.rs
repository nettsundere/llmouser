//! LLMouser: native shells (AppKit, GTK/libadwaita, Win32) around the
//! `llmouser-browser` crate. Unsafe code lives only in the platform modules,
//! where it wraps the OS toolkits.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod session;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

fn main() {
    #[cfg(target_os = "macos")]
    macos::run();
    #[cfg(target_os = "linux")]
    linux::run();
    #[cfg(target_os = "windows")]
    windows::run();
}
