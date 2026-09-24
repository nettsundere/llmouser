use crate::harness::{App, Cmd};

#[test]
fn settings_window_has_llm_and_ux_tabs_with_labels() {
    let mut app = App::launch();
    assert!(!app.settings().open);
    let info = app.open_settings();
    assert_eq!(info.title, "Settings");
    assert_eq!(info.tab, "llm");
    assert_eq!(info.tab_labels, ["LLM Settings", "UX"]);
    assert_eq!(
        info.labels,
        [
            "Provider",
            "Endpoint",
            "Model",
            "API Key",
            "Max output tokens",
            "Universe rules",
            "Language",
            "Search URL"
        ]
    );
    assert_eq!(info.provider, "openai");
    assert_eq!(info.provider_options, ["OpenAI", "Anthropic"]);
    assert_eq!(info.endpoint, "https://api.openai.com/v1");
    assert_eq!(info.model, "gpt-4o");
    assert_eq!(info.api_key, "");
    assert_eq!(info.api_key_placeholder, "Enter API key");
    assert_eq!(info.max_tokens, "64000");
    assert_eq!(info.language, "en");
    assert_eq!(info.language_options, ["English", "Русский", "中文"]);
    assert_eq!(info.search_url, "https://www.google.com/search?q={query}");
    assert_eq!(info.save_label, "Save");
    assert_eq!(info.close_label, "Close");
    app.settings_tab("ux");
    assert_eq!(app.settings().tab, "ux");
    assert!(app.state().settings_open);
    app.ok(Cmd::SettingsClose);
    app.wait_for("settings closed", |a| !a.settings().open);
}

#[test]
fn switching_provider_prefills_endpoint_and_model_defaults() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("provider", "anthropic");
    let info = app.settings();
    assert_eq!(info.endpoint, "https://api.anthropic.com");
    assert_eq!(info.model, "claude-sonnet-5");
    app.settings_set("provider", "openai");
    let info = app.settings();
    assert_eq!(info.endpoint, "https://api.openai.com/v1");
    assert_eq!(info.model, "gpt-4o");
}

#[test]
fn saved_settings_persist_across_restarts_without_exposing_the_key() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("provider", "anthropic");
    app.settings_set("model", "claude-custom");
    app.settings_set("api_key", "sk-secret");
    app.settings_set("max_tokens", "1234");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    app.go("example.com");
    assert_eq!(app.text("mock-provider"), "anthropic");

    let mut app = app.relaunch();
    let info = app.open_settings();
    assert_eq!(info.provider, "anthropic");
    assert_eq!(info.model, "claude-custom");
    assert_eq!(info.max_tokens, "1234");
    assert_eq!(info.api_key, "");
    assert_eq!(
        info.api_key_placeholder,
        "Saved — leave blank to keep, type to replace"
    );
    let stored =
        std::fs::read_to_string(app.data_path().join("llm-browser-settings.json")).unwrap();
    assert!(stored.contains("sk-secret"));
}

#[test]
fn invalid_values_are_explained_and_keep_the_window_open() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("max_tokens", "lots");
    app.settings_save();
    let info = app.settings();
    assert!(info.open);
    assert_eq!(
        info.validation,
        "Max output tokens must be a whole number above zero"
    );
    app.settings_set("max_tokens", "10");
    app.settings_set("endpoint", "ftp://nope");
    app.settings_save();
    assert_eq!(app.settings().validation, "Endpoint must be an http(s) URL");
    app.settings_set("endpoint", "");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    let info = app.open_settings();
    assert_eq!(info.endpoint, "https://api.openai.com/v1");
    assert_eq!(info.max_tokens, "10");
}

#[test]
fn close_discards_unsaved_edits() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("model", "unsaved-model");
    app.ok(Cmd::SettingsClose);
    app.wait_for("settings closed", |a| !a.settings().open);
    let info = app.open_settings();
    assert_eq!(info.model, "gpt-4o");
}
