use crate::harness::App;

#[test]
fn relative_link_generates_the_target_page() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-link-relative");
    let state = app.wait_status("Loaded https://example.com/about");
    assert_eq!(state.address, "https://example.com/about");
    assert_eq!(app.text("mock-url"), "https://example.com/about");
    assert_eq!(app.text("mock-referer"), "https://example.com/");
    assert!(state.can_back);
}

#[test]
fn absolute_link_to_another_site_generates_that_site() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-link-absolute");
    app.wait_status("Loaded https://other.example/page");
    assert_eq!(app.text("mock-url"), "https://other.example/page");
    assert_eq!(app.text("mock-referer"), "https://example.com/");
}

#[test]
fn js_driven_navigation_is_regenerated_instead_of_loaded() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-js-nav");
    app.wait_status("Loaded https://example.com/js-nav");
    assert_eq!(app.text("mock-url"), "https://example.com/js-nav");
}

#[test]
fn js_navigation_to_an_absolute_url_never_reaches_the_network() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-js-nav-absolute");
    app.wait_status("Loaded https://real.example/leak");
    assert_eq!(app.text("mock-url"), "https://real.example/leak");
    assert_eq!(app.text("mock-provider"), "openai");
}

#[test]
fn hash_link_stays_on_the_current_page() {
    let mut app = App::launch();
    app.go("example.com");
    app.page_click("mock-link-hash");
    std::thread::sleep(std::time::Duration::from_millis(600));
    let state = app.state();
    assert_eq!(state.status, "Loaded example.com");
    assert!(!state.can_back);
    assert_eq!(app.text("mock-url"), "https://example.com/");
}

#[test]
fn target_blank_links_open_in_the_same_tab() {
    let mut app = App::launch();
    app.go("example.com");
    let tabs_before = app.tabs().len();
    app.page_click("mock-link-blank");
    app.wait_status("Loaded https://example.com/popup");
    assert_eq!(app.tabs().len(), tabs_before);
    assert_eq!(app.text("mock-url"), "https://example.com/popup");
}

#[test]
fn search_form_submits_and_session_context_chains_across_sites() {
    let mut app = App::launch();
    app.go("example.com");
    app.eval("(function(){ document.getElementById('mock-search-input').value = 'cats'; document.getElementById('mock-search-submit').click(); return true })()");
    app.wait_status("Loaded https://example.com/search?q=cats");
    assert_eq!(app.text("mock-referer"), "https://example.com/");
    let params =
        app.eval_str("document.querySelector('#mock-params li[data-param=q]').textContent");
    assert_eq!(params, "q=cats");

    app.page_click("mock-link-absolute");
    app.wait_status("Loaded https://other.example/page");
    assert_eq!(
        app.text("mock-referer"),
        "https://example.com/search?q=cats"
    );
    assert_eq!(
        app.text("mock-history"),
        "https://example.com/, https://example.com/search?q=cats"
    );
}
