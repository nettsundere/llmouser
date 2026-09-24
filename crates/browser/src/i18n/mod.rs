//! Every user-visible string of the browser chrome, per language.
//!
//! Strings carry `{placeholders}` filled by [`fill`]; the shells never
//! concatenate translated fragments themselves.

use serde::{Deserialize, Serialize};

mod en;
mod ru;
mod zh;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ru,
    Zh,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::En, Language::Ru, Language::Zh];

    pub fn id(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Ru => "ru",
            Language::Zh => "zh",
        }
    }

    pub fn from_id(id: &str) -> Option<Language> {
        match id {
            "en" => Some(Language::En),
            "ru" => Some(Language::Ru),
            "zh" => Some(Language::Zh),
            _ => None,
        }
    }

    /// First-run default: follow the OS locale; the settings selector overrides it.
    pub fn detect() -> Language {
        sys_locale::get_locale()
            .map(|l| Language::from_locale(&l))
            .unwrap_or(Language::En)
    }

    pub fn from_locale(locale: &str) -> Language {
        let lower = locale.to_ascii_lowercase();
        if lower.starts_with("ru") {
            Language::Ru
        } else if lower.starts_with("zh") {
            Language::Zh
        } else {
            Language::En
        }
    }

    pub fn messages(self) -> &'static Messages {
        match self {
            Language::En => &en::EN,
            Language::Ru => &ru::RU,
            Language::Zh => &zh::ZH,
        }
    }
}

/// Fill `{name}` placeholders in a translated template.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// All languages, indexed like [`Language::ALL`].
pub static MESSAGES: [&Messages; 3] = [&en::EN, &ru::RU, &zh::ZH];

#[derive(Debug, Clone, Serialize)]
pub struct Messages {
    /// Name of the language, written in the language itself.
    pub language_name: &'static str,

    // Toolbar
    pub back: &'static str,
    pub forward: &'static str,
    pub go: &'static str,
    pub stop: &'static str,
    pub settings: &'static str,
    pub address_bar: &'static str,
    pub address_placeholder: &'static str,
    pub new_tab: &'static str,
    pub close_tab: &'static str,
    pub rendered_site: &'static str,

    // Error page shown in the viewport
    pub open_settings: &'static str,
    pub error_title: &'static str,
    pub retry: &'static str,

    // Settings panel
    pub settings_title: &'static str,
    pub settings_tab_llm: &'static str,
    pub settings_tab_ux: &'static str,
    pub provider: &'static str,
    pub endpoint: &'static str,
    pub model: &'static str,
    pub api_key: &'static str,
    pub api_key_enter: &'static str,
    pub api_key_saved: &'static str,
    pub max_tokens: &'static str,
    pub universe_rules: &'static str,
    pub universe_placeholder: &'static str,
    pub language: &'static str,
    pub search_url: &'static str,
    pub search_url_hint: &'static str,
    pub save: &'static str,
    pub close: &'static str,
    pub invalid_max_tokens: &'static str,
    pub invalid_endpoint: &'static str,
    pub invalid_search_url: &'static str,

    // Status line
    pub ready: &'static str,
    pub loading: &'static str,
    pub loaded: &'static str,
    pub failed_to_load: &'static str,
    pub stopped: &'static str,
    pub saving_pdf: &'static str,
    pub saved_pdf: &'static str,
    pub pdf_canceled: &'static str,
    pub failed_pdf: &'static str,
    pub saved_html: &'static str,
    pub html_canceled: &'static str,
    pub failed_html: &'static str,
    pub nothing_to_save: &'static str,
    pub error_no_api_key: &'static str,

    // Application menu
    pub menu_file: &'static str,
    pub menu_new_tab: &'static str,
    pub menu_new_window: &'static str,
    pub menu_close_tab: &'static str,
    pub menu_reopen_closed_tab: &'static str,
    pub menu_save_pdf: &'static str,
    pub menu_save_html: &'static str,
    pub menu_close_window: &'static str,
    pub menu_quit: &'static str,
    pub menu_exit: &'static str,

    pub menu_about: &'static str,
    pub about_version: &'static str,
    pub menu_settings: &'static str,
    pub menu_services: &'static str,
    pub menu_hide: &'static str,
    pub menu_hide_others: &'static str,
    pub menu_show_all: &'static str,

    pub menu_edit: &'static str,
    pub menu_undo: &'static str,
    pub menu_redo: &'static str,
    pub menu_cut: &'static str,
    pub menu_copy: &'static str,
    pub menu_paste: &'static str,
    pub menu_paste_match_style: &'static str,
    pub menu_delete: &'static str,
    pub menu_select_all: &'static str,

    pub menu_view: &'static str,
    pub menu_reload: &'static str,
    pub menu_stop: &'static str,
    pub menu_reset_zoom: &'static str,
    pub menu_zoom_in: &'static str,
    pub menu_zoom_out: &'static str,
    pub menu_toggle_fullscreen: &'static str,
    pub menu_open_location: &'static str,

    pub menu_history: &'static str,
    pub menu_back: &'static str,
    pub menu_forward: &'static str,

    pub menu_window: &'static str,
    pub menu_minimize: &'static str,
    pub menu_zoom_window: &'static str,
    pub menu_front: &'static str,
    pub menu_show_next_tab: &'static str,
    pub menu_show_previous_tab: &'static str,

    pub menu_help: &'static str,
    pub menu_homepage: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_fill() {
        assert_eq!(fill("Loading {url}…", &[("url", "a.b")]), "Loading a.b…");
        assert_eq!(
            fill(en::EN.failed_to_load, &[("url", "x"), ("error", "boom")]),
            "Failed to load x: boom"
        );
    }

    #[test]
    fn every_language_fills_the_same_placeholders() {
        for lang in Language::ALL {
            let m = lang.messages();
            for (name, value) in [
                ("loading", m.loading),
                ("loaded", m.loaded),
                ("stopped", m.stopped),
                ("error_title", m.error_title),
            ] {
                assert!(value.contains("{url}"), "{name} in {lang:?} lacks {{url}}");
            }
            assert!(m.failed_to_load.contains("{url}") && m.failed_to_load.contains("{error}"));
            assert!(m.saved_pdf.contains("{path}"));
            assert!(m.saved_html.contains("{path}"));
            assert!(m.failed_pdf.contains("{error}"));
            assert!(m.failed_html.contains("{error}"));
            assert!(m.menu_quit.contains("{app}"));
            assert!(m.menu_about.contains("{app}"));
            assert!(m.menu_hide.contains("{app}"));
        }
    }

    #[test]
    fn locale_detection() {
        assert_eq!(Language::from_locale("ru-RU"), Language::Ru);
        assert_eq!(Language::from_locale("zh_CN.UTF-8"), Language::Zh);
        assert_eq!(Language::from_locale("en-US"), Language::En);
        assert_eq!(Language::from_locale("de"), Language::En);
    }
}
