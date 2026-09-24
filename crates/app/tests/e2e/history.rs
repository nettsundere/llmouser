use crate::harness::{modifier, App};

#[test]
fn back_and_forward_restore_cached_pages() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    let state = app.state();
    assert!(state.can_back && !state.can_forward);

    app.click("back");
    let state = app.wait_status("Loaded a.example");
    assert_eq!(state.address, "a.example");
    assert!(!state.can_back && state.can_forward);
    assert_eq!(app.text("mock-url"), "https://a.example/");
    // Cached: the referer shown is the one from the original generation.
    assert_eq!(app.text("mock-referer"), "none");

    app.click("forward");
    let state = app.wait_status("Loaded b.example");
    assert!(state.can_back && !state.can_forward);
    assert_eq!(app.text("mock-referer"), "https://a.example/");
}

#[test]
fn navigating_from_mid_history_discards_forward_entries() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    app.click("back");
    app.wait_status("Loaded a.example");
    app.go("c.example");
    let state = app.state();
    assert!(state.can_back && !state.can_forward);
    assert_eq!(app.text("mock-history"), "https://a.example/");
}

#[test]
fn link_clicks_add_history_entries() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-link-relative");
    app.wait_status("Loaded https://example.com/about");
    app.click("back");
    app.wait_status("Loaded example.com");
    assert_eq!(app.text("mock-url"), "https://example.com/");
    app.click("forward");
    app.wait_status("Loaded https://example.com/about");
}

#[test]
fn history_is_tracked_per_tab() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    app.new_tab();
    let state = app.state();
    assert!(!state.can_back && !state.can_forward);
    app.go("c.example");
    assert_eq!(app.text("mock-referer"), "none");
    app.activate_tab(0);
    let state = app.state();
    assert!(state.can_back);
    assert_eq!(state.address, "b.example");
}

#[test]
fn keyboard_shortcuts_go_back_and_forward() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    app.key(&format!("{}+[", modifier()));
    app.wait_status("Loaded a.example");
    app.key(&format!("{}+]", modifier()));
    app.wait_status("Loaded b.example");
}

#[test]
fn history_menu_items_reflect_the_position() {
    let mut app = App::launch();
    let menu = app.menu();
    let back = crate::harness::find_menu(&menu, "back").expect("back item");
    assert!(!back.enabled);
    app.go("a.example");
    app.go("b.example");
    let menu = app.menu();
    assert!(crate::harness::find_menu(&menu, "back").unwrap().enabled);
    assert!(!crate::harness::find_menu(&menu, "forward").unwrap().enabled);
    app.menu_click("back");
    app.wait_status("Loaded a.example");
    let menu = app.menu();
    assert!(crate::harness::find_menu(&menu, "forward").unwrap().enabled);
    app.menu_click("forward");
    app.wait_status("Loaded b.example");
}
