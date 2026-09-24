//! Platform-independent glue between the browser model, the settings store and
//! the engine. Every shell drives the app through this type; it never touches
//! widgets, so it is unit-tested here and shared by all three platforms.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use llmouser_browser::browser::{CloseOutcome, NavigateOptions};
use llmouser_browser::engine::EventSink;
use llmouser_browser::i18n::Messages;
use llmouser_browser::page;
use llmouser_browser::settings::{Provider, SettingsUpdate};
use llmouser_browser::{
    Browser, Content, Engine, EngineEvent, Language, PublicSettings, SettingsStore, Status, TabId,
};

pub struct Session {
    pub browser: Browser,
    pub store: SettingsStore,
    pub engine: Engine,
}

/// What the settings dialog collected, before validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsForm {
    pub provider: Provider,
    pub endpoint: String,
    pub model: String,
    pub api_key: String,
    pub max_tokens: String,
    pub universe: String,
    pub language: Language,
    pub search_url: String,
}

impl Session {
    pub fn new(sink: EventSink) -> Session {
        let mock = Engine::mock_from_env();
        Session {
            browser: Browser::new(),
            store: SettingsStore::open_default(),
            engine: Engine::new(mock, sink),
        }
    }

    #[cfg(test)]
    fn with_store(store: SettingsStore, mock: bool, sink: EventSink) -> Session {
        Session {
            browser: Browser::new(),
            store,
            engine: Engine::new(mock, sink),
        }
    }

    pub fn language(&self) -> Language {
        self.store.language()
    }

    pub fn messages(&self) -> &'static Messages {
        self.store.language().messages()
    }

    pub fn public_settings(&self) -> PublicSettings {
        self.store.public()
    }

    /// Whether navigation can produce pages right now.
    pub fn can_generate(&self) -> bool {
        self.engine.is_mock() || self.store.settings().has_api_key()
    }

    fn run(&mut self, generation: Option<llmouser_browser::Generation>) {
        if let Some(g) = generation {
            self.engine.start(g, self.store.settings().clone());
        }
    }

    /// The address bar was submitted.
    pub fn go(&mut self, tab: TabId, input: &str) {
        let search = self.store.settings().search_url.clone();
        let can = self.can_generate();
        let m = self.messages();
        let g = self.browser.navigate(
            tab,
            input,
            NavigateOptions {
                search_url: &search,
                can_generate: can,
                messages: m,
            },
        );
        self.run(g);
    }

    /// A link or form inside the page.
    pub fn open_link(&mut self, tab: TabId, href: &str) {
        let search = self.store.settings().search_url.clone();
        let can = self.can_generate();
        let m = self.messages();
        let g = self.browser.open_link(
            tab,
            href,
            NavigateOptions {
                search_url: &search,
                can_generate: can,
                messages: m,
            },
        );
        self.run(g);
    }

    /// Regenerate the current page or retry a failed one.
    pub fn reload(&mut self, tab: TabId) {
        let search = self.store.settings().search_url.clone();
        let can = self.can_generate();
        let m = self.messages();
        let g = self.browser.reload(
            tab,
            NavigateOptions {
                search_url: &search,
                can_generate: can,
                messages: m,
            },
        );
        self.run(g);
    }

    pub fn stop(&mut self, tab: TabId) {
        if let Some(request) = self.browser.cancel(tab) {
            self.engine.cancel(request);
        }
    }

    pub fn back(&mut self, tab: TabId) {
        if let Some(request) = self.browser.back(tab) {
            self.engine.cancel(request);
        }
    }

    pub fn forward(&mut self, tab: TabId) {
        if let Some(request) = self.browser.forward(tab) {
            self.engine.cancel(request);
        }
    }

    pub fn close_tab(&mut self, tab: TabId) -> CloseOutcome {
        let outcome = self.browser.close_tab(tab);
        if let Some(request) = outcome.cancelled {
            self.engine.cancel(request);
        }
        outcome
    }

    /// Apply an engine event. Returns the tab whose view changed, if any.
    pub fn on_event(&mut self, event: EngineEvent) -> Option<TabId> {
        match event {
            EngineEvent::Generated {
                tab,
                request,
                result,
            } => self.browser.finish(tab, request, result).then_some(tab),
        }
    }

    /// HTML the shell should show for a tab's content.
    pub fn document_for(&self, tab: TabId, inject_base: bool) -> Option<(String, Option<String>)> {
        let t = self.browser.tab(tab)?;
        let m = self.messages();
        Some(match &t.content {
            Content::Empty => (page::start_page(), None),
            Content::Page { url, html } => (
                page::prepare_document(html, inject_base.then_some(url.as_str())),
                Some(url.clone()),
            ),
            Content::Error {
                url,
                message,
                offer_settings,
            } => (page::error_page(m, url, message, *offer_settings), None),
        })
    }

    /// The form as the settings dialog should show it (never the key itself).
    pub fn settings_form(&self) -> SettingsForm {
        let s = self.store.settings();
        SettingsForm {
            provider: s.provider,
            endpoint: s.endpoint.clone(),
            model: s.model.clone(),
            api_key: String::new(),
            max_tokens: s.max_tokens.to_string(),
            universe: s.universe.clone(),
            language: s.language,
            search_url: s.search_url.clone(),
        }
    }

    /// Validate a form; the message to show, if anything is wrong.
    pub fn validate(&self, form: &SettingsForm) -> Option<&'static str> {
        let m = self.messages();
        let endpoint = form.endpoint.trim();
        if !endpoint.is_empty()
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            return Some(m.invalid_endpoint);
        }
        let tokens = form.max_tokens.trim();
        if !tokens.is_empty() && tokens.parse::<u32>().map(|n| n == 0).unwrap_or(true) {
            return Some(m.invalid_max_tokens);
        }
        let search = form.search_url.trim();
        if !search.is_empty() && !search.contains(llmouser_browser::url::QUERY_PLACEHOLDER) {
            return Some(m.invalid_search_url);
        }
        None
    }

    /// Save a validated form. Returns the language that is now active.
    pub fn save_settings(&mut self, form: &SettingsForm) -> Result<PublicSettings, String> {
        if let Some(problem) = self.validate(form) {
            return Err(problem.to_string());
        }
        let update = SettingsUpdate {
            provider: form.provider,
            endpoint: form.endpoint.clone(),
            model: form.model.clone(),
            api_key: (!form.api_key.trim().is_empty()).then(|| form.api_key.clone()),
            universe: form.universe.clone(),
            max_tokens: form.max_tokens.trim().parse().ok(),
            language: form.language,
            search_url: form.search_url.clone(),
        };
        self.store.save(update).map_err(|e| e.to_string())
    }

    /// The instant language switch: persists only the language.
    pub fn set_language(&mut self, language: Language) -> Result<(), String> {
        let update = SettingsUpdate::language_only(self.store.settings(), language);
        self.store
            .save(update)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Page of a tab for saving, or the "nothing to save" status when empty.
    pub fn page_to_save(&mut self, tab: TabId) -> Option<(String, String)> {
        let page = self
            .browser
            .tab(tab)
            .and_then(|t| t.page_html().map(|(u, h)| (u.to_string(), h.to_string())));
        if page.is_none() {
            self.browser.set_status(tab, Status::NothingToSave);
        }
        page
    }

    pub fn set_status(&mut self, tab: TabId, status: Status) {
        self.browser.set_status(tab, status);
    }

    /// Native save dialogs are stubbed under test: `Some(None)` means the stub
    /// cancelled, `Some(Some(path))` means it chose a path, `None` means "ask".
    pub fn save_dialog_stub() -> Option<Option<PathBuf>> {
        let value = std::env::var(llmouser_browser::env::SAVE_PATH).ok()?;
        Some((!value.is_empty()).then(|| PathBuf::from(value)))
    }

    /// Write a page's HTML to disk, with the same CSP the viewer applies.
    pub fn write_html(path: &Path, url: &str, html: &str) -> std::io::Result<()> {
        std::fs::write(path, page::prepare_document(html, Some(url)))
    }
}

/// Turn the page URL into a friendly default file name.
pub fn default_file_name(url: &str, extension: &str) -> String {
    let base = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| format!("{h}{}", u.path())))
        .unwrap_or_else(|| url.to_string());
    let mut cleaned = String::new();
    let mut last_dash = true;
    for c in base.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            cleaned.push(c);
            last_dash = false;
        } else if !last_dash {
            cleaned.push('-');
            last_dash = true;
        }
    }
    let cleaned = cleaned.trim_matches('-');
    format!(
        "{}.{extension}",
        if cleaned.is_empty() { "page" } else { cleaned }
    )
}

/// Sink that forwards engine events through a channel-like callback.
pub fn sink(f: impl Fn(EngineEvent) + Send + Sync + 'static) -> EventSink {
    Arc::new(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::sync::Mutex;
    use std::time::Duration;

    fn session(mock: bool) -> (Session, mpsc::Receiver<EngineEvent>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let store = SettingsStore::open(dir.path(), None);
        let s = Session::with_store(
            store,
            mock,
            sink(move |e| {
                let _ = tx.lock().unwrap().send(e);
            }),
        );
        (s, rx, dir)
    }

    #[test]
    fn go_runs_the_engine_and_applies_the_result() {
        let (mut s, rx, _dir) = session(true);
        let tab = s.browser.active_id();
        s.go(tab, "example.com");
        assert!(s.browser.active().is_loading());
        let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(s.on_event(event), Some(tab));
        assert_eq!(s.browser.active().title, "Mock: https://example.com/");
        let (doc, base) = s.document_for(tab, false).unwrap();
        assert!(doc.contains("Content-Security-Policy") && !doc.contains("<base"));
        assert_eq!(base.as_deref(), Some("https://example.com/"));
        let (doc, _) = s.document_for(tab, true).unwrap();
        assert!(doc.contains("<base href=\"https://example.com/\">"));

        s.open_link(tab, "/about");
        let event = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        s.on_event(event);
        assert_eq!(s.browser.active().url, "https://example.com/about");
        s.back(tab);
        assert_eq!(s.browser.active().url, "https://example.com/");
        s.forward(tab);
        assert_eq!(s.browser.active().url, "https://example.com/about");
        s.reload(tab);
        assert!(s.browser.active().is_loading());
        s.stop(tab);
        assert!(!s.browser.active().is_loading());
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    }

    #[test]
    fn without_a_key_navigation_is_refused_with_guidance() {
        let (mut s, rx, _dir) = session(false);
        assert!(!s.can_generate());
        let tab = s.browser.active_id();
        s.go(tab, "example.com");
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
        assert!(matches!(
            s.browser.active().content,
            Content::Error {
                offer_settings: true,
                ..
            }
        ));
        let (doc, _) = s.document_for(tab, false).unwrap();
        assert!(doc.contains("No API key configured"));
    }

    #[test]
    fn settings_validation_and_language_switch() {
        let (mut s, _rx, _dir) = session(false);
        let mut form = s.settings_form();
        assert_eq!(form.max_tokens, "64000");
        assert_eq!(form.api_key, "");
        form.endpoint = "ftp://x".into();
        assert_eq!(s.validate(&form), Some("Endpoint must be an http(s) URL"));
        form.endpoint = "https://ok".into();
        form.max_tokens = "abc".into();
        assert_eq!(
            s.validate(&form),
            Some("Max output tokens must be a whole number above zero")
        );
        form.max_tokens = "0".into();
        assert!(s.validate(&form).is_some());
        form.max_tokens = "".into();
        form.search_url = "https://x".into();
        assert_eq!(s.validate(&form), Some("Search URL must contain {query}"));
        form.search_url = "".into();
        form.api_key = " key ".into();
        form.provider = Provider::Anthropic;
        let saved = s.save_settings(&form).unwrap();
        assert!(saved.has_api_key);
        assert_eq!(saved.provider, Provider::Anthropic);
        assert_eq!(saved.max_tokens, 64000);
        assert!(s.can_generate());

        s.set_language(Language::Ru).unwrap();
        assert_eq!(s.language(), Language::Ru);
        assert_eq!(s.store.settings().api_key, "key");
        form.max_tokens = "x".into();
        assert_eq!(
            s.save_settings(&form).unwrap_err(),
            "Максимум выходных токенов должен быть целым числом больше нуля"
        );
    }

    #[test]
    fn page_to_save_and_file_names() {
        let (mut s, rx, _dir) = session(true);
        let tab = s.browser.active_id();
        assert!(s.page_to_save(tab).is_none());
        assert_eq!(s.browser.active().status, Status::NothingToSave);
        s.go(tab, "example.com/a-b");
        s.on_event(rx.recv_timeout(Duration::from_secs(5)).unwrap());
        let (url, html) = s.page_to_save(tab).unwrap();
        assert_eq!(url, "https://example.com/a-b");
        assert!(html.contains("Mock page"));
        assert_eq!(default_file_name(&url, "pdf"), "example-com-a-b.pdf");
        assert_eq!(default_file_name("garbage", "html"), "garbage.html");
        assert_eq!(default_file_name("https://x.y/", "pdf"), "x-y.pdf");
        assert_eq!(default_file_name("///", "pdf"), "page.pdf");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.html");
        Session::write_html(&path, &url, &html).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("<base href=\"https://example.com/a-b\">"));
    }

    #[test]
    fn save_dialog_stub_reads_env() {
        // Serialise env mutation with a lock shared by tests in this module.
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().unwrap();
        std::env::remove_var(llmouser_browser::env::SAVE_PATH);
        assert_eq!(Session::save_dialog_stub(), None);
        std::env::set_var(llmouser_browser::env::SAVE_PATH, "");
        assert_eq!(Session::save_dialog_stub(), Some(None));
        std::env::set_var(llmouser_browser::env::SAVE_PATH, "/tmp/x.pdf");
        assert_eq!(
            Session::save_dialog_stub(),
            Some(Some(PathBuf::from("/tmp/x.pdf")))
        );
        std::env::remove_var(llmouser_browser::env::SAVE_PATH);
    }
}
