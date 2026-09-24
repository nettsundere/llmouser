use crate::harness::{App, Cmd};

#[test]
fn right_click_offers_edit_actions_and_save_commands() {
    let mut app = App::launch();
    app.go("example.com");
    let items = app.context_menu();
    let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["cut", "copy", "paste", "save-pdf", "save-html"]);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Cut",
            "Copy",
            "Paste",
            "Save Page as PDF…",
            "Save Page as HTML…"
        ]
    );
    assert!(items[3].enabled && items[4].enabled);
}

#[test]
fn context_menu_follows_the_ui_language() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("language", "ru");
    app.ok(Cmd::SettingsClose);
    let labels: Vec<String> = app.context_menu().into_iter().map(|i| i.label).collect();
    assert_eq!(
        labels,
        [
            "Вырезать",
            "Копировать",
            "Вставить",
            "Сохранить страницу как PDF…",
            "Сохранить страницу как HTML…"
        ]
    );
}

#[test]
fn context_menu_save_acts_like_the_app_menu() {
    let mut app = App::launch();
    app.context_menu();
    app.ok(Cmd::ContextMenuClick {
        id: "save-pdf".into(),
    });
    assert_eq!(app.wait_status("Nothing to save").status, "Nothing to save");
}
