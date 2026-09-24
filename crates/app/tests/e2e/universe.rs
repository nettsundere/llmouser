use crate::harness::App;

const DEFAULT: &str =
    "Our real universe, exactly as it is today: real sites, brands, people and events.";

#[test]
fn sites_are_generated_in_our_universe_by_default() {
    let mut app = App::launch();
    app.go("example.com");
    assert_eq!(app.text("mock-universe"), DEFAULT);
    assert_eq!(app.open_settings().universe, DEFAULT);
}

#[test]
fn redefining_the_universe_alters_generation_and_persists() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("universe", "A universe where cats run every company.");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    app.go("example.com");
    assert_eq!(
        app.text("mock-universe"),
        "A universe where cats run every company."
    );
    let mut app = app.relaunch();
    assert_eq!(
        app.open_settings().universe,
        "A universe where cats run every company."
    );
}

#[test]
fn clearing_the_universe_field_restores_the_default() {
    let mut app = App::launch();
    app.open_settings();
    app.settings_set("universe", "Mars");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    app.open_settings();
    app.settings_set("universe", "   ");
    app.settings_save();
    app.wait_for("settings closed", |a| !a.settings().open);
    app.go("example.com");
    assert_eq!(app.text("mock-universe"), DEFAULT);
}
