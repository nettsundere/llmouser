//! Persistent settings. The file format is the one the Electron versions wrote
//! (`llm-browser-settings.json`, camelCase keys), so an existing store is picked up
//! on first run instead of asking the user to re-enter their API key.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::i18n::Language;
use crate::url::DEFAULT_SEARCH_URL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    OpenAi,
    Anthropic,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::OpenAi, Provider::Anthropic];

    pub fn id(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
        }
    }

    pub fn from_id(id: &str) -> Option<Provider> {
        match id {
            "openai" => Some(Provider::OpenAi),
            "anthropic" => Some(Provider::Anthropic),
            _ => None,
        }
    }

    /// Human-readable name shown in the provider selector.
    pub fn label(self) -> &'static str {
        match self {
            Provider::OpenAi => "OpenAI",
            Provider::Anthropic => "Anthropic",
        }
    }

    pub fn default_endpoint(self) -> &'static str {
        match self {
            Provider::OpenAi => "https://api.openai.com/v1",
            Provider::Anthropic => "https://api.anthropic.com",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Provider::OpenAi => "gpt-4o",
            Provider::Anthropic => "claude-sonnet-5",
        }
    }
}

/// The universe websites are generated in, unless the user redefines it.
pub const DEFAULT_UNIVERSE: &str =
    "Our real universe, exactly as it is today: real sites, brands, people and events.";

/// Safe across providers (e.g. Anthropic rejects max_tokens above the model's limit).
pub const DEFAULT_MAX_TOKENS: u32 = 64000;

/// File name shared with the Electron versions so their settings carry over.
pub const SETTINGS_FILE: &str = "llm-browser-settings.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub provider: Provider,
    /// Base URL of the API, e.g. https://api.openai.com/v1 or https://api.anthropic.com
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    /// Description of the universe all generated sites exist in.
    pub universe: String,
    /// Output token cap sent as max_tokens on every generation request.
    pub max_tokens: u32,
    /// UI language of the browser chrome.
    pub language: Language,
    /// Template used when address-bar text is a search query, with `{query}`.
    pub search_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings::defaults_for(Language::detect())
    }
}

impl Settings {
    pub fn defaults_for(language: Language) -> Self {
        Settings {
            provider: Provider::OpenAi,
            endpoint: Provider::OpenAi.default_endpoint().to_string(),
            api_key: String::new(),
            model: Provider::OpenAi.default_model().to_string(),
            universe: DEFAULT_UNIVERSE.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            language,
            search_url: DEFAULT_SEARCH_URL.to_string(),
        }
    }

    /// Fill in anything an older or hand-edited file left empty.
    fn sanitized(mut self) -> Self {
        if self.endpoint.trim().is_empty() {
            self.endpoint = self.provider.default_endpoint().to_string();
        }
        if self.model.trim().is_empty() {
            self.model = self.provider.default_model().to_string();
        }
        if self.universe.trim().is_empty() {
            self.universe = DEFAULT_UNIVERSE.to_string();
        }
        if self.max_tokens == 0 {
            self.max_tokens = DEFAULT_MAX_TOKENS;
        }
        if !self.search_url.contains(crate::url::QUERY_PLACEHOLDER) {
            self.search_url = DEFAULT_SEARCH_URL.to_string();
        }
        self
    }

    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// View without the key itself — what the UI is allowed to see.
    pub fn public(&self) -> PublicSettings {
        PublicSettings {
            provider: self.provider,
            endpoint: self.endpoint.clone(),
            model: self.model.clone(),
            has_api_key: self.has_api_key(),
            universe: self.universe.clone(),
            max_tokens: self.max_tokens,
            language: self.language,
            search_url: self.search_url.clone(),
        }
    }

    /// Apply a UI update. Empty fields fall back to defaults; an empty key keeps
    /// the stored one (the UI never shows the saved value).
    pub fn apply(&mut self, update: SettingsUpdate) {
        self.provider = update.provider;
        self.endpoint = non_empty(&update.endpoint)
            .unwrap_or_else(|| update.provider.default_endpoint().to_string());
        self.model =
            non_empty(&update.model).unwrap_or_else(|| update.provider.default_model().to_string());
        self.universe = non_empty(&update.universe).unwrap_or_else(|| DEFAULT_UNIVERSE.to_string());
        self.max_tokens = update
            .max_tokens
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_MAX_TOKENS);
        self.language = update.language;
        self.search_url = non_empty(&update.search_url)
            .filter(|s| s.contains(crate::url::QUERY_PLACEHOLDER))
            .unwrap_or_else(|| DEFAULT_SEARCH_URL.to_string());
        if let Some(key) = update.api_key.as_deref().and_then(non_empty) {
            self.api_key = key;
        }
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Settings as exposed to the UI — the API key itself never leaves the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSettings {
    pub provider: Provider,
    pub endpoint: String,
    pub model: String,
    pub has_api_key: bool,
    pub universe: String,
    pub max_tokens: u32,
    pub language: Language,
    pub search_url: String,
}

/// Settings update from the UI. `None`/empty `api_key` keeps the stored key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsUpdate {
    pub provider: Provider,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub universe: String,
    pub max_tokens: Option<u32>,
    pub language: Language,
    pub search_url: String,
}

impl SettingsUpdate {
    /// An update that changes nothing but the language (the instant language switch).
    pub fn language_only(current: &Settings, language: Language) -> Self {
        SettingsUpdate {
            provider: current.provider,
            endpoint: current.endpoint.clone(),
            model: current.model.clone(),
            api_key: None,
            universe: current.universe.clone(),
            max_tokens: Some(current.max_tokens),
            language,
            search_url: current.search_url.clone(),
        }
    }
}

/// Where settings live and how they get there.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    /// Resolve the directory: `LLMOUSER_DATA_DIR` if set, else the OS config
    /// directory for the app. On first run the Electron store, if present, is
    /// imported from its old location.
    pub fn open_default() -> SettingsStore {
        // An explicit data directory (tests, portable installs) is isolated: it
        // never inherits the old Electron store.
        match std::env::var_os(crate::env::DATA_DIR) {
            Some(dir) => SettingsStore::open(&PathBuf::from(dir), None),
            None => SettingsStore::open(&default_dir(), legacy_electron_dir().as_deref()),
        }
    }

    pub fn open(dir: &Path, legacy_dir: Option<&Path>) -> SettingsStore {
        let path = dir.join(SETTINGS_FILE);
        let settings = load(&path)
            .or_else(|| {
                legacy_dir
                    .map(|d| d.join(SETTINGS_FILE))
                    .and_then(|p| load(&p))
            })
            .unwrap_or_default();
        SettingsStore { path, settings }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn public(&self) -> PublicSettings {
        self.settings.public()
    }

    pub fn language(&self) -> Language {
        self.settings.language
    }

    /// Apply and persist. Returns the new public view.
    pub fn save(&mut self, update: SettingsUpdate) -> Result<PublicSettings, std::io::Error> {
        self.settings.apply(update);
        self.persist()?;
        Ok(self.settings.public())
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        // Write-then-rename so a crash never leaves a half-written key behind.
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)
    }
}

fn load(path: &Path) -> Option<Settings> {
    let text = fs::read_to_string(path).ok()?;
    let settings: Settings = serde_json::from_str(&text).ok()?;
    Some(settings.sanitized())
}

/// The per-user configuration directory of this OS (no extra crates: the
/// conventions are three environment variables).
pub fn config_base_dir() -> Option<PathBuf> {
    let env = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support"))
    } else if cfg!(target_os = "windows") {
        env("APPDATA")
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))
    }
}

fn default_dir() -> PathBuf {
    config_base_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("llmouser")
}

/// Electron's `userData` directory for the previous versions of the app.
fn legacy_electron_dir() -> Option<PathBuf> {
    config_base_dir().map(|b| b.join("LLMouser"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_openai() {
        let s = Settings::defaults_for(Language::En);
        assert_eq!(s.provider, Provider::OpenAi);
        assert_eq!(s.endpoint, "https://api.openai.com/v1");
        assert_eq!(s.model, "gpt-4o");
        assert!(!s.has_api_key());
        assert_eq!(s.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn apply_fills_defaults_and_keeps_key() {
        let mut s = Settings::defaults_for(Language::En);
        s.api_key = "secret".into();
        s.apply(SettingsUpdate {
            provider: Provider::Anthropic,
            endpoint: "  ".into(),
            model: "".into(),
            api_key: Some("".into()),
            universe: "".into(),
            max_tokens: Some(0),
            language: Language::Ru,
            search_url: "nope".into(),
        });
        assert_eq!(s.provider, Provider::Anthropic);
        assert_eq!(s.endpoint, "https://api.anthropic.com");
        assert_eq!(s.model, "claude-sonnet-5");
        assert_eq!(s.api_key, "secret");
        assert_eq!(s.universe, DEFAULT_UNIVERSE);
        assert_eq!(s.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(s.language, Language::Ru);
        assert_eq!(s.search_url, DEFAULT_SEARCH_URL);

        s.apply(SettingsUpdate {
            api_key: Some(" new-key ".into()),
            max_tokens: Some(1234),
            universe: " Mars ".into(),
            ..SettingsUpdate::language_only(&s, Language::Zh)
        });
        assert_eq!(s.api_key, "new-key");
        assert_eq!(s.max_tokens, 1234);
        assert_eq!(s.universe, "Mars");
        assert_eq!(s.language, Language::Zh);
        assert!(s.public().has_api_key);
    }

    #[test]
    fn store_round_trips_and_never_exposes_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SettingsStore::open(dir.path(), None);
        let public = store
            .save(SettingsUpdate {
                api_key: Some("k".into()),
                ..SettingsUpdate::language_only(store.settings(), Language::En)
            })
            .unwrap();
        assert!(public.has_api_key);
        let json = fs::read_to_string(store.path()).unwrap();
        assert!(json.contains("\"apiKey\": \"k\""));
        assert!(serde_json::to_string(&public)
            .unwrap()
            .contains("hasApiKey"));
        assert!(!serde_json::to_string(&public).unwrap().contains("\"k\""));

        let reopened = SettingsStore::open(dir.path(), None);
        assert_eq!(reopened.settings(), store.settings());
    }

    #[test]
    fn imports_electron_store_on_first_run() {
        let legacy = tempfile::tempdir().unwrap();
        let fresh = tempfile::tempdir().unwrap();
        fs::write(
            legacy.path().join(SETTINGS_FILE),
            r#"{"provider":"anthropic","endpoint":"https://api.anthropic.com","apiKey":"old",
                "model":"claude-sonnet-5","universe":"","maxTokens":0,"language":"zh"}"#,
        )
        .unwrap();
        let store = SettingsStore::open(fresh.path(), Some(legacy.path()));
        assert_eq!(store.settings().provider, Provider::Anthropic);
        assert_eq!(store.settings().api_key, "old");
        assert_eq!(store.settings().universe, DEFAULT_UNIVERSE);
        assert_eq!(store.settings().max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(store.settings().language, Language::Zh);
        assert_eq!(store.settings().search_url, DEFAULT_SEARCH_URL);
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(SETTINGS_FILE), "{not json").unwrap();
        let store = SettingsStore::open(dir.path(), None);
        assert_eq!(store.settings().provider, Provider::OpenAi);
    }
}
