use crate::harness::App;

#[test]
fn shows_an_error_state_when_generation_fails() {
    let mut app = App::launch();
    app.go("example.com");
    let state = app.go("example.com/throw-error");
    assert_eq!(
        state.status,
        "Failed to load example.com/throw-error: Mock provider forced error"
    );
    assert!(state.status_error);
    assert_eq!(
        app.text("error-title"),
        "Couldn't load https://example.com/throw-error"
    );
    assert_eq!(app.text("error-detail"), "Mock provider forced error");
    assert_eq!(
        app.eval_str("document.getElementById('open-settings') ? 'yes' : 'no'"),
        "no"
    );
    // The failed page is not part of history.
    assert!(!state.can_back);
}

#[test]
fn retry_regenerates_the_failed_page() {
    let mut app = App::launch();
    app.go("example.com/throw-error");
    app.page_click("retry");
    let state = app.wait_settled();
    assert!(state.status_error);
    assert_eq!(
        app.text("error-title"),
        "Couldn't load https://example.com/throw-error"
    );
    let menu = app.menu();
    assert!(crate::harness::find_menu(&menu, "reload").unwrap().enabled);
    app.menu_click("reload");
    app.wait_settled();
    assert_eq!(app.text("error-detail"), "Mock provider forced error");
}

#[test]
fn credential_errors_offer_the_settings() {
    let mut app = App::launch();
    let state = app.go("example.com/unauthorized");
    assert_eq!(
        state.status,
        "Failed to load example.com/unauthorized: Mock request failed (401): bad key"
    );
    assert_eq!(
        app.eval_str("document.getElementById('open-settings') ? 'yes' : 'no'"),
        "yes"
    );
    app.page_click("open-settings");
    app.wait_for("settings window", |a| a.settings().open);
}

#[test]
fn error_pages_are_translated() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("language", "ru");
    app.ok(crate::harness::Cmd::SettingsClose);
    let state = app.go("example.com/throw-error");
    assert_eq!(
        state.status,
        "Не удалось загрузить example.com/throw-error: Mock provider forced error"
    );
    assert_eq!(
        app.text("error-title"),
        "Не удалось загрузить https://example.com/throw-error"
    );
    assert_eq!(app.text("retry"), "Повторить");
}
