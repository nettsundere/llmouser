use crate::harness::{App, Launch};

#[test]
fn saving_with_no_page_reports_nothing_to_save() {
    let mut app = App::launch();
    app.menu_click("save-pdf");
    assert_eq!(app.wait_status("Nothing to save").status, "Nothing to save");
    app.menu_click("save-html");
    assert_eq!(app.state().status, "Nothing to save");
}

#[test]
fn saves_the_current_page_as_a_pdf_file() {
    let out = tempfile::tempdir().unwrap();
    let path = out.path().join("example-com.pdf");
    let mut app = App::launch_with(Launch {
        save_path: Some(path.display().to_string()),
        ..Launch::default()
    });
    app.go("example.com");
    app.menu_click("save-pdf");
    let state = app.wait_status_contains("Saved PDF: ");
    assert!(state.status.ends_with("example-com.pdf"));
    assert!(!state.status_error);
    let bytes = std::fs::read(&path).expect("pdf written");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "not a PDF: {:?}",
        &bytes[..8.min(bytes.len())]
    );
}

#[test]
fn cancelling_the_save_dialog_leaves_the_page_untouched() {
    let mut app = App::launch_with(Launch {
        save_path: Some(String::new()),
        ..Launch::default()
    });
    app.go("example.com");
    app.menu_click("save-pdf");
    assert_eq!(
        app.wait_status("PDF save canceled").status,
        "PDF save canceled"
    );
    assert_eq!(app.text("mock-url"), "https://example.com/");
    app.menu_click("save-html");
    assert_eq!(
        app.wait_status("Page save canceled").status,
        "Page save canceled"
    );
}

#[test]
fn saves_the_current_page_as_html() {
    let out = tempfile::tempdir().unwrap();
    let path = out.path().join("page.html");
    let mut app = App::launch_with(Launch {
        save_path: Some(path.display().to_string()),
        ..Launch::default()
    });
    app.go("example.com");
    app.menu_click("save-html");
    let state = app.wait_status_contains("Saved page: ");
    assert!(state.status.ends_with("page.html"));
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.contains("<base href=\"https://example.com/\">"));
    assert!(html.contains("Content-Security-Policy"));
    assert!(html.contains("Mock page for https://example.com/"));
}
