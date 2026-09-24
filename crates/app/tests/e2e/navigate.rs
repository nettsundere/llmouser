use crate::harness::App;

#[test]
fn generates_and_renders_a_page_from_the_address_bar() {
    let mut app = App::launch();
    let state = app.go("example.com");
    assert_eq!(state.status, "Loaded example.com");
    assert!(!state.status_error);
    assert_eq!(state.address, "example.com");
    assert_eq!(app.text("mock-url"), "https://example.com/");
    assert_eq!(
        app.text("mock-heading"),
        "Mock page for https://example.com/"
    );
    assert_eq!(app.active_title(), "Mock: https://example.com/");
    assert_eq!(app.state().window_title, "Mock: https://example.com/");
}

#[test]
fn prepends_https_to_a_bare_host_and_keeps_explicit_schemes() {
    let mut app = App::launch();
    app.go("example.com/path?x=1");
    assert_eq!(app.text("mock-url"), "https://example.com/path?x=1");
    app.go("http://plain.example/");
    assert_eq!(app.text("mock-url"), "http://plain.example/");
}

#[test]
fn the_go_button_submits_the_address() {
    let mut app = App::launch();
    app.set_address("button.example");
    app.click("go");
    let state = app.wait_settled();
    assert_eq!(state.status, "Loaded button.example");
    assert_eq!(app.text("mock-url"), "https://button.example/");
}

#[test]
fn empty_address_does_nothing() {
    let mut app = App::launch();
    app.set_address("   ");
    app.submit();
    let state = app.state();
    assert_eq!(state.status, "Ready");
    assert!(!state.loading);
}

#[test]
fn loading_shows_status_and_a_stop_control() {
    let mut app = App::launch();
    app.go("example.com");
    app.set_address("slow.example");
    app.submit();
    let state = app.state();
    assert!(state.loading);
    assert_eq!(state.status, "Loading https://slow.example/…");
    assert_eq!(state.tooltips[2], "Stop");
    assert!(state.tabs[state.active_tab].loading);

    app.click("stop");
    let state = app.wait_settled();
    assert!(!state.loading);
    assert_eq!(state.status, "Stopped loading https://slow.example/");
    assert_eq!(state.tooltips[2], "Go");
    // The previous page is still shown.
    assert_eq!(app.text("mock-url"), "https://example.com/");
}

#[test]
fn fenced_llm_output_is_stripped_before_rendering() {
    // A real LLM may answer with a prose preamble and a ```html fence despite
    // being asked for raw HTML. The browser must render the page, not the prose.
    let mut app = App::launch();
    let state = app.go("fenced.example");
    assert_eq!(state.status, "Loaded fenced.example");
    assert_eq!(app.text("mock-url"), "https://fenced.example/");
    assert_eq!(
        app.eval_str(
            "document.body.textContent.indexOf('Here is the HTML code') >= 0 ? 'yes' : 'no'"
        ),
        "no"
    );
    assert_eq!(app.state().window_title, "Mock: https://fenced.example/");
}
