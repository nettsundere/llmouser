use crate::harness::App;

#[test]
fn generated_pages_cannot_load_external_resources() {
    let mut app = App::launch();
    app.go("example.com");
    app.wait_for("image verdict", |a| {
        !a.eval_str("document.getElementById('mock-ext-img').getAttribute('data-net') || ''")
            .is_empty()
    });
    assert_eq!(
        app.eval_str("document.getElementById('mock-ext-img').getAttribute('data-net')"),
        "blocked"
    );
}

#[test]
fn generated_pages_cannot_fetch_from_the_network() {
    let mut app = App::launch();
    app.go("example.com");
    app.wait_for("fetch verdict", |a| {
        a.text("mock-fetch-result") != "pending"
    });
    assert_eq!(app.text("mock-fetch-result"), "fetch-blocked");
}

#[test]
fn nested_frames_and_scripts_from_the_network_are_blocked() {
    let mut app = App::launch();
    app.go("example.com");
    let loaded = app.eval_str(
        "(function(){ var f = document.createElement('iframe'); f.src = 'https://leak.example/'; document.body.appendChild(f); \
         var s = document.createElement('script'); s.src = 'https://leak.example/x.js'; document.body.appendChild(s); return 'added' })()",
    );
    assert_eq!(loaded, "added");
    std::thread::sleep(std::time::Duration::from_millis(800));
    // Still our page, nothing navigated or executed from outside.
    assert_eq!(app.text("mock-url"), "https://example.com/");
    assert_eq!(app.state().status, "Loaded example.com");
}
