use crate::harness::{App, Cmd};

#[test]
fn about_window_shows_a_centered_icon_version_and_copyright() {
    let mut app = App::launch();
    assert!(!app.about().open);
    app.menu_click("about");
    app.wait_for("about window", |a| a.about().open);
    let about = app.about();
    assert_eq!(about.title, "About LLMouser");
    assert_eq!(about.name, "LLMouser");
    assert_eq!(
        about.version_line,
        format!("Version {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(about.copyright, "© 2026 Vladimir Kiselev. MIT License");
    assert!(about.icon_loaded);
    assert!(
        about.icon_off_center < 2.0,
        "icon off center by {}",
        about.icon_off_center
    );
    assert!(app.state().about_open);
    app.ok(Cmd::AboutClose);
    app.wait_for("about closed", |a| !a.about().open);
}

#[test]
fn about_window_follows_the_ui_language() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("language", "ru");
    app.ok(Cmd::SettingsClose);
    app.menu_click("about");
    app.wait_for("about window", |a| a.about().open);
    let about = app.about();
    assert_eq!(about.title, "О программе LLMouser");
    assert_eq!(
        about.version_line,
        format!("Версия {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(about.copyright, "© 2026 Vladimir Kiselev. MIT License");
}
