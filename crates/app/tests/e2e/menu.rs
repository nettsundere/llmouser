use crate::harness::{find_menu, is_macos, modifier, App};

#[test]
fn application_menu_has_the_standard_structure_and_shortcuts() {
    let mut app = App::launch();
    let menu = app.menu();
    let top: Vec<&str> = menu.iter().map(|m| m.id.as_str()).collect();
    if is_macos() {
        assert_eq!(
            top,
            ["app", "file", "edit", "view", "history", "window", "help"]
        );
        assert_eq!(find_menu(&menu, "quit").unwrap().label, "Quit LLMouser");
        assert_eq!(find_menu(&menu, "about").unwrap().label, "About LLMouser");
        assert_eq!(find_menu(&menu, "hide").unwrap().label, "Hide LLMouser");
        assert!(find_menu(&menu, "next-tab").is_some());
    } else {
        assert_eq!(top, ["file", "edit", "view", "history", "help"]);
        assert_eq!(find_menu(&menu, "quit").unwrap().label, "Exit");
        assert!(find_menu(&menu, "about").is_some());
    }
    let m = modifier();
    assert_eq!(
        find_menu(&menu, "new-tab").unwrap().accelerator,
        format!("{m}+t")
    );
    assert_eq!(
        find_menu(&menu, "close-tab").unwrap().accelerator,
        format!("{m}+w")
    );
    assert_eq!(
        find_menu(&menu, "save-pdf").unwrap().accelerator,
        format!("{m}+s")
    );
    assert_eq!(
        find_menu(&menu, "reload").unwrap().accelerator,
        format!("{m}+r")
    );
    assert_eq!(
        find_menu(&menu, "back").unwrap().accelerator,
        if is_macos() {
            "cmd+[".to_string()
        } else {
            "alt+left".to_string()
        }
    );
    assert_eq!(find_menu(&menu, "settings").unwrap().label, "Settings…");
    for id in [
        "undo",
        "redo",
        "cut",
        "copy",
        "paste",
        "select-all",
        "zoom-in",
        "zoom-out",
        "reset-zoom",
        "homepage",
    ] {
        assert!(find_menu(&menu, id).is_some(), "missing {id}");
    }
}

#[test]
fn new_tab_menu_item_opens_a_tab() {
    let mut app = App::launch();
    app.menu_click("new-tab");
    app.wait_for("second tab", |a| a.tabs().len() == 2);
    assert_eq!(app.state().active_tab, 1);
    app.menu_click("close-tab");
    app.wait_for("tab closed", |a| a.tabs().len() == 1);
}

#[test]
fn zoom_menu_items_change_the_page_zoom() {
    let mut app = App::launch();
    app.go("example.com");
    // Page zoom scales CSS pixels: zooming in shrinks the CSS viewport.
    let base: f64 = app.eval("window.innerWidth").as_f64().unwrap();
    app.menu_click("zoom-in");
    app.wait_for("zoomed in", |a| {
        a.eval("window.innerWidth").as_f64().unwrap() < base * 0.95
    });
    app.menu_click("zoom-out");
    app.menu_click("zoom-out");
    app.wait_for("zoomed out", |a| {
        a.eval("window.innerWidth").as_f64().unwrap() > base * 1.05
    });
    app.menu_click("reset-zoom");
    app.wait_for("zoom reset", |a| {
        (a.eval("window.innerWidth").as_f64().unwrap() - base).abs() < 2.0
    });
}
