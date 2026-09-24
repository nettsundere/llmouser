use crate::harness::App;

#[test]
fn plain_words_search_through_the_configured_engine() {
    let mut app = App::launch();
    app.go("best cat food");
    assert_eq!(
        app.text("mock-url"),
        "https://www.google.com/search?q=best+cat+food"
    );
    let params = app.eval_str("Array.from(document.querySelectorAll('#mock-params li')).map(function (li) { return li.textContent }).join('|')");
    assert_eq!(params, "q=best cat food");
}

#[test]
fn the_search_url_is_configurable() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("search_url", "https://duck.example/?q={query}");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    app.go("cats");
    assert_eq!(app.text("mock-url"), "https://duck.example/?q=cats");

    // Templates without the placeholder are refused.
    app.open_settings();
    app.settings_tab("ux");
    app.settings_set("search_url", "https://broken.example/");
    app.settings_save();
    let info = app.settings();
    assert!(info.open);
    assert_eq!(info.validation, "Search URL must contain {query}");
}
