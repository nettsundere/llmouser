//! The browser itself, minus pixels.
//!
//! Everything a browser needs except pixels lives here: the tab/history model
//! ([`browser`]), settings persistence ([`settings`]), the LLM providers that
//! play the role of "the internet" ([`llm`]), page preparation ([`page`]),
//! translations ([`i18n`]), the async request engine ([`engine`]) and the test
//! automation protocol ([`automation`]). The platform shells in the `llmouser`
//! binary only render this state with native widgets and forward user intent.
//!
//! This crate forbids `unsafe`; platform FFI is confined to the native shells in the `llmouser` binary.
#![forbid(unsafe_code)]

pub mod automation;
pub mod browser;
pub mod engine;
pub mod i18n;
pub mod llm;
pub mod page;
pub mod prompt;
pub mod settings;
pub mod url;

pub use browser::{Browser, Content, Generation, RequestId, Status, Tab, TabId};
pub use engine::{Engine, EngineEvent};
pub use i18n::{Language, Messages, MESSAGES};
pub use settings::{Provider, PublicSettings, Settings, SettingsStore, SettingsUpdate};

/// Application display name.
pub const APP_NAME: &str = "LLMouser";
/// Version string shown in the About window (from the workspace manifest).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Copyright line shown in the About window.
pub const APP_COPYRIGHT: &str = "© 2026 Vladimir Kiselev. MIT License";
/// Project homepage.
pub const APP_HOMEPAGE: &str = "https://github.com/nettsundere/llmouser";

/// Environment variable names honoured by the app (shared with the E2E harness).
pub mod env {
    /// `1` selects the deterministic offline provider instead of a real LLM.
    pub const MOCK: &str = "LLMOUSER_MOCK";
    /// Directory holding `settings.json`; defaults to the OS config directory.
    pub const DATA_DIR: &str = "LLMOUSER_DATA_DIR";
    /// TCP port for the automation server used by the E2E suite.
    pub const E2E_PORT: &str = "LLMOUSER_E2E_PORT";
    /// Stubs native save dialogs under test: a path means "the user chose this
    /// file", an empty value means "the user cancelled".
    pub const SAVE_PATH: &str = "LLMOUSER_SAVE_PATH";
}
