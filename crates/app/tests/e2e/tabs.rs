use crate::harness::{tabs_are_windows, App};

#[test]
fn tabs_open_independent_sites_with_titles_and_icons() {
    let mut app = App::launch();
    app.go("first.example");
    app.new_tab();
    let tabs = app.tabs();
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[1].title, "New Tab");
    assert!(tabs[1].active);
    app.go("second.example");
    let tabs = app.tabs();
    assert_eq!(tabs[0].title, "Mock: https://first.example/");
    assert_eq!(tabs[1].title, "Mock: https://second.example/");
    if !tabs_are_windows() {
        assert_eq!(tabs[0].icon, "favicon");
        assert_eq!(tabs[1].icon, "favicon");
    }
    assert_eq!(app.text("mock-url"), "https://second.example/");
    app.activate_tab(0);
    assert_eq!(app.text("mock-url"), "https://first.example/");
}

#[test]
fn tabs_have_independent_history_and_referer_context() {
    let mut app = App::launch();
    app.go("a.example");
    app.page_click("mock-link-relative");
    app.wait_status("Loaded https://a.example/about");
    app.new_tab();
    app.go("b.example");
    assert_eq!(app.text("mock-referer"), "none");
    assert!(!app.state().can_back);
    app.activate_tab(0);
    assert!(app.state().can_back);
    assert_eq!(app.text("mock-referer"), "https://a.example/");
}

#[test]
fn closing_tabs_activates_a_neighbour() {
    let mut app = App::launch();
    app.go("a.example");
    app.new_tab();
    app.go("b.example");
    app.new_tab();
    app.go("c.example");
    app.activate_tab(0);
    app.close_tab(0);
    let state = app.state();
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(
        state.tabs[state.active_tab].title,
        "Mock: https://b.example/"
    );
    app.close_tab(1);
    let state = app.state();
    assert_eq!(state.tabs.len(), 1);
    assert_eq!(state.tabs[0].title, "Mock: https://b.example/");
}

#[test]
fn closing_the_last_tab_follows_platform_conventions() {
    let mut app = App::launch();
    app.go("a.example");
    app.close_tab(0);
    if tabs_are_windows() {
        // macOS: the window is gone; the app keeps running and New Tab reopens one.
        assert!(app.state().tabs.is_empty());
        app.menu_click("new-window");
        app.wait_for("new window", |a| a.tabs().len() == 1);
        assert_eq!(app.state().address, "");
    } else {
        let state = app.state();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.tabs[0].title, "New Tab");
        assert_eq!(state.address, "");
    }
}

#[test]
fn typed_address_text_survives_switching_tabs() {
    let mut app = App::launch();
    app.set_address("draft text");
    app.new_tab();
    assert_eq!(app.state().address, "");
    app.activate_tab(0);
    assert_eq!(app.state().address, "draft text");
}

#[test]
fn closed_tabs_can_be_reopened_with_their_history() {
    let mut app = App::launch();
    let menu = app.menu();
    assert!(
        !crate::harness::find_menu(&menu, "reopen-closed-tab")
            .unwrap()
            .enabled
    );
    app.go("a.example");
    app.go("b.example");
    app.new_tab();
    app.activate_tab(0);
    app.close_tab(0);
    let menu = app.menu();
    assert!(
        crate::harness::find_menu(&menu, "reopen-closed-tab")
            .unwrap()
            .enabled
    );
    app.menu_click("reopen-closed-tab");
    app.wait_for("reopened tab", |a| a.tabs().len() == 2);
    app.wait_for("reopened tab active", |a| {
        a.active_title() == "Mock: https://b.example/"
    });
    let state = app.wait_settled();
    assert!(state.can_back);
    assert_eq!(state.address, "b.example");
    assert_eq!(app.text("mock-url"), "https://b.example/");
}

#[test]
fn falls_back_to_a_letter_icon_without_a_favicon() {
    if tabs_are_windows() {
        return; // macOS window tabs have no icons.
    }
    let mut app = App::launch();
    app.go("noicon.example");
    let tabs = app.tabs();
    assert_eq!(tabs[0].icon, "letter");
    app.new_tab();
    assert_eq!(app.tabs()[1].icon, "blank");
}
