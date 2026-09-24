use crate::harness::{App, Launch};

#[test]
fn a_new_tab_is_blank_like_a_browser() {
    let mut app = App::launch();
    assert!(app.on_start_page());
    assert_eq!(app.eval_str("document.body.textContent.trim()"), "");
    assert_eq!(app.state().status, "Ready");
    app.new_tab();
    assert!(app.on_start_page());
}

#[test]
fn without_an_api_key_navigation_fails_fast_and_leads_to_settings() {
    let mut app = App::launch_with(Launch {
        mock: false,
        ..Launch::default()
    });
    let state = app.go("example.com");
    assert_eq!(
        state.status,
        "Failed to load https://example.com/: No API key configured. Open Settings to add one."
    );
    assert!(state.status_error);
    assert!(!state.loading);
    assert_eq!(
        app.eval_str("document.getElementById('open-settings') ? 'yes' : 'no'"),
        "yes"
    );
    app.page_click("open-settings");
    app.wait_for("settings window", |a| a.settings().open);
    app.settings_set("api_key", "sk-test");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    // With a key the page is regenerated on retry (real provider: no network here, so it fails differently).
    app.page_click("retry");
    app.wait_for("retry started", |a| {
        let s = a.state();
        s.loading || !s.status.contains("No API key")
    });
}
