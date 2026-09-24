use crate::harness::{modifier, App};

#[test]
fn keyboard_shortcuts_open_and_close_tabs() {
    let mut app = App::launch();
    app.key(&format!("{}+t", modifier()));
    app.wait_for("second tab", |a| a.tabs().len() == 2);
    assert_eq!(app.state().address, "");
    app.key(&format!("{}+w", modifier()));
    app.wait_for("tab closed", |a| a.tabs().len() == 1);
}

#[test]
fn keyboard_shortcut_regenerates_and_stops() {
    let mut app = App::launch();
    app.go("example.com");
    app.key(&format!("{}+r", modifier()));
    app.wait_for("regenerating", |a| {
        a.state().loading || a.state().status == "Loaded example.com"
    });
    app.wait_settled();
    assert_eq!(app.text("mock-url"), "https://example.com/");
    app.set_address("slow.example");
    app.submit();
    app.key(&format!("{}+.", modifier()));
    let state = app.wait_settled();
    assert_eq!(state.status, "Stopped loading https://slow.example/");
}
