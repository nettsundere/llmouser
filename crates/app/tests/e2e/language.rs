use crate::harness::{find_menu, App, Cmd};

#[test]
fn language_switch_retranslates_the_whole_chrome_immediately() {
    let mut app = App::launch();
    app.go("example.com");
    app.new_tab();
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("language", "ru");
    let info = app.settings();
    assert_eq!(info.title, "Настройки");
    assert_eq!(info.tab_labels, ["Настройки LLM", "Интерфейс"]);
    assert_eq!(info.labels[0], "Провайдер");
    assert_eq!(info.save_label, "Сохранить");
    assert_eq!(info.api_key_placeholder, "Введите API-ключ");

    let state = app.state();
    assert_eq!(state.address_placeholder, "Введите запрос или адрес сайта");
    assert_eq!(state.tooltips, ["Назад", "Вперёд", "Перейти", "Настройки"]);
    assert_eq!(state.status, "Готово");
    assert_eq!(state.tabs[1].title, "Новая вкладка");
    assert_eq!(state.tabs[0].title, "Mock: https://example.com/");

    let menu = app.menu();
    assert_eq!(find_menu(&menu, "file").unwrap().label, "Файл");
    assert_eq!(
        find_menu(&menu, "save-pdf").unwrap().label,
        "Сохранить страницу как PDF…"
    );
    assert_eq!(find_menu(&menu, "back").unwrap().label, "Назад");

    // The status of the other tab is retranslated too.
    app.activate_tab(0);
    assert_eq!(app.state().status, "Загружено: example.com");

    app.settings_set("language", "zh");
    assert_eq!(app.state().tooltips[0], "后退");
    assert_eq!(find_menu(&app.menu(), "file").unwrap().label, "文件");
    app.settings_set("language", "en");
    assert_eq!(app.state().tooltips[0], "Back");
}

#[test]
fn language_persists_without_save_and_leaves_unsaved_edits_uncommitted() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("model", "not-saved");
    app.settings_tab("ux");
    app.settings_set("language", "zh");
    app.ok(Cmd::SettingsClose);
    let mut app = app.relaunch();
    let state = app.state();
    assert_eq!(state.status, "就绪");
    let info = app.open_settings();
    assert_eq!(info.language, "zh");
    assert_eq!(info.model, "gpt-4o");
}
