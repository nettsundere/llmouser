use crate::harness::App;

#[test]
fn back_during_load_then_a_new_request_cancels_the_old_one() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    app.set_address("slow.example");
    app.submit();
    assert!(app.state().loading);
    app.click("back");
    let state = app.wait_status("Loaded a.example");
    assert!(!state.loading);
    app.go("c.example");
    assert_eq!(app.text("mock-url"), "https://c.example/");
    // The slow result never arrives late.
    std::thread::sleep(std::time::Duration::from_millis(3500));
    assert_eq!(app.text("mock-url"), "https://c.example/");
    assert_eq!(app.state().status, "Loaded c.example");
}

#[test]
fn concurrent_requests_in_different_tabs_do_not_cancel_each_other() {
    let mut app = App::launch();
    app.set_address("slow.example");
    app.submit();
    app.new_tab();
    app.set_address("slow2.example");
    app.submit();
    assert!(app.state().loading);
    app.activate_tab(0);
    assert!(app.state().loading);
    app.wait_status("Loaded slow.example");
    assert_eq!(app.text("mock-url"), "https://slow.example/");
    app.activate_tab(1);
    app.wait_status("Loaded slow2.example");
    assert_eq!(app.text("mock-url"), "https://slow2.example/");
}

#[test]
fn back_alone_during_load_discards_the_in_flight_result() {
    let mut app = App::launch();
    app.go("a.example");
    app.go("b.example");
    app.set_address("slow.example");
    app.submit();
    app.click("back");
    app.wait_status("Loaded a.example");
    std::thread::sleep(std::time::Duration::from_millis(3500));
    let state = app.state();
    assert_eq!(state.status, "Loaded a.example");
    assert_eq!(state.address, "a.example");
    assert_eq!(app.text("mock-url"), "https://a.example/");
    assert!(state.can_forward);
}

#[test]
fn stop_menu_item_and_shortcut_cancel_loading() {
    let mut app = App::launch();
    app.set_address("slow.example");
    app.submit();
    let menu = app.menu();
    assert!(crate::harness::find_menu(&menu, "stop").unwrap().enabled);
    app.menu_click("stop");
    let state = app.wait_settled();
    assert_eq!(state.status, "Stopped loading https://slow.example/");
    let menu = app.menu();
    assert!(!crate::harness::find_menu(&menu, "stop").unwrap().enabled);
}
