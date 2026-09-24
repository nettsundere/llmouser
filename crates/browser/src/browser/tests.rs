use super::*;
use crate::i18n::Language;
use crate::url::DEFAULT_SEARCH_URL;

fn opts() -> NavigateOptions<'static> {
    NavigateOptions {
        search_url: DEFAULT_SEARCH_URL,
        can_generate: true,
        messages: Language::En.messages(),
    }
}

fn page(title: &str) -> String {
    format!("<html><head><title>{title}</title></head><body>{title}</body></html>")
}

#[test]
fn navigate_then_finish_records_history() {
    let mut b = Browser::new();
    let id = b.active_id();
    let g = b.navigate(id, " example.com ", opts()).unwrap();
    assert_eq!(g.site.url, "https://example.com/");
    assert_eq!(g.site.referer, None);
    assert!(b.active().is_loading());
    assert_eq!(
        b.active().status,
        Status::Loading {
            url: "https://example.com/".into()
        }
    );
    assert_eq!(b.active().address, "example.com");

    assert!(b.finish(id, g.request, Ok(page("Example"))));
    let tab = b.active();
    assert!(!tab.is_loading());
    assert_eq!(tab.title, "Example");
    assert_eq!(tab.url, "https://example.com/");
    assert_eq!(
        tab.status,
        Status::Loaded {
            url: "example.com".into()
        }
    );
    assert!(matches!(&tab.content, Content::Page { url, .. } if url == "https://example.com/"));
    assert!(!tab.can_back() && !tab.can_forward());

    let g2 = b.navigate(id, "cats and dogs", opts()).unwrap();
    assert_eq!(g2.site.url, "https://www.google.com/search?q=cats+and+dogs");
    assert_eq!(g2.site.referer.as_deref(), Some("https://example.com/"));
    assert_eq!(g2.site.history, vec!["https://example.com/"]);
    b.finish(id, g2.request, Ok(page("Search")));
    assert!(b.active().can_back());
}

#[test]
fn late_results_are_discarded() {
    let mut b = Browser::new();
    let id = b.active_id();
    let g1 = b.navigate(id, "a.com", opts()).unwrap();
    let g2 = b.navigate(id, "b.com", opts()).unwrap();
    assert!(!b.finish(id, g1.request, Ok(page("A"))));
    assert!(b.finish(id, g2.request, Ok(page("B"))));
    assert_eq!(b.active().title, "B");
    assert_eq!(b.active().entries().len(), 1);
}

#[test]
fn cancel_stops_and_keeps_page() {
    let mut b = Browser::new();
    let id = b.active_id();
    let g = b.navigate(id, "a.com", opts()).unwrap();
    b.finish(id, g.request, Ok(page("A")));
    let g2 = b.navigate(id, "slow.com", opts()).unwrap();
    assert_eq!(b.cancel(id), Some(g2.request));
    assert_eq!(b.cancel(id), None);
    assert_eq!(
        b.active().status,
        Status::Stopped {
            url: "https://slow.com/".into()
        }
    );
    assert!(!b.finish(id, g2.request, Ok(page("Slow"))));
    assert_eq!(b.active().title, "A");
}

#[test]
fn errors_render_an_error_page() {
    let mut b = Browser::new();
    let id = b.active_id();
    let g = b.navigate(id, "a.com/throw-error", opts()).unwrap();
    b.finish(id, g.request, Err(GenerateError::Other("boom".into())));
    let tab = b.active();
    assert_eq!(
        tab.status,
        Status::Failed {
            url: "a.com/throw-error".into(),
            error: "boom".into()
        }
    );
    assert!(tab.status.is_error());
    assert!(
        matches!(&tab.content, Content::Error { offer_settings: false, message, .. } if message == "boom")
    );
    assert!(tab.entries().is_empty());

    // Retry regenerates the failed URL.
    let g = b.reload(id, opts()).unwrap();
    assert_eq!(g.site.url, "https://a.com/throw-error");
    b.finish(
        id,
        g.request,
        Err(GenerateError::Http {
            provider: "X",
            status: 401,
            body: "k".into(),
        }),
    );
    assert!(matches!(
        &b.active().content,
        Content::Error {
            offer_settings: true,
            ..
        }
    ));
}

#[test]
fn missing_key_fails_fast_with_guidance() {
    let mut b = Browser::new();
    let id = b.active_id();
    let no_key = NavigateOptions {
        can_generate: false,
        ..opts()
    };
    assert!(b.navigate(id, "a.com", no_key).is_none());
    let tab = b.active();
    assert!(!tab.is_loading());
    assert_eq!(
        tab.status.text(Language::En.messages()),
        "Failed to load https://a.com/: No API key configured. Open Settings to add one."
    );
    assert!(matches!(
        &tab.content,
        Content::Error {
            offer_settings: true,
            ..
        }
    ));
}

#[test]
fn back_forward_restore_cached_pages_and_truncate() {
    let mut b = Browser::new();
    let id = b.active_id();
    for name in ["a.com", "b.com", "c.com"] {
        let g = b.navigate(id, name, opts()).unwrap();
        b.finish(id, g.request, Ok(page(name)));
    }
    assert_eq!(b.active().content_seq, 3);
    assert!(b.back(id).is_none());
    assert_eq!(b.active().title, "b.com");
    assert_eq!(b.active().address, "b.com");
    assert!(b.active().can_forward() && b.active().can_back());
    assert_eq!(b.active().content_seq, 4);
    b.back(id);
    assert_eq!(b.active().title, "a.com");
    assert!(!b.active().can_back());
    assert!(b.back(id).is_none());
    b.forward(id);
    assert_eq!(b.active().title, "b.com");

    // Navigating from mid-history discards forward entries.
    let g = b.navigate(id, "d.com", opts()).unwrap();
    assert_eq!(g.site.history, vec!["https://a.com/", "https://b.com/"]);
    b.finish(id, g.request, Ok(page("d.com")));
    assert_eq!(
        b.active()
            .entries()
            .iter()
            .map(|e| e.title.as_str())
            .collect::<Vec<_>>(),
        ["a.com", "b.com", "d.com"]
    );
    assert!(!b.active().can_forward());
}

#[test]
fn back_during_load_abandons_the_request() {
    let mut b = Browser::new();
    let id = b.active_id();
    for name in ["a.com", "b.com"] {
        let g = b.navigate(id, name, opts()).unwrap();
        b.finish(id, g.request, Ok(page(name)));
    }
    let g = b.navigate(id, "slow.com", opts()).unwrap();
    assert_eq!(b.back(id), Some(g.request));
    assert_eq!(b.active().title, "a.com");
    assert!(!b.finish(id, g.request, Ok(page("slow"))));
    assert_eq!(b.active().title, "a.com");
}

#[test]
fn tabs_are_independent_and_close_activates_neighbour() {
    let mut b = Browser::new();
    let first = b.active_id();
    let g = b.navigate(first, "a.com", opts()).unwrap();
    b.finish(first, g.request, Ok(page("A")));
    let second = b.new_tab();
    assert_eq!(b.active_id(), second);
    let g = b.navigate(second, "b.com", opts()).unwrap();
    assert_eq!(g.site.referer, None);
    b.finish(second, g.request, Ok(page("B")));
    let third = b.new_tab();
    b.set_address(third, "draft");
    assert_eq!(b.tab(third).unwrap().address, "draft");
    assert!(b.activate(first));
    assert!(!b.activate(999));

    let g = b.navigate(first, "slow.com", opts()).unwrap();
    let outcome = b.close_tab(first);
    assert_eq!(
        outcome,
        CloseOutcome {
            cancelled: Some(g.request),
            activated: Some(second)
        }
    );
    assert_eq!(b.tabs().len(), 2);
    b.activate(third);
    assert_eq!(
        b.close_tab(second),
        CloseOutcome {
            cancelled: None,
            activated: Some(third)
        }
    );
    assert_eq!(
        b.close_tab(third),
        CloseOutcome {
            cancelled: None,
            activated: None
        }
    );
    assert!(b.tabs().is_empty());
    assert_eq!(
        b.close_tab(42),
        CloseOutcome {
            cancelled: None,
            activated: None
        }
    );

    // Reopen brings back the last closed tab that had pages (the second).
    let reopened = b.reopen_closed().unwrap();
    assert_eq!(b.active_id(), reopened);
    assert_eq!(b.active().title, "B");
    assert_eq!(b.active().address, "b.com");
    let reopened = b.reopen_closed().unwrap();
    assert_eq!(b.tab(reopened).unwrap().title, "A");
    assert!(b.reopen_closed().is_none());
}

#[test]
fn links_resolve_against_the_page() {
    assert_eq!(
        resolve_link("https://a.com/x/y", "/about").as_deref(),
        Some("https://a.com/about")
    );
    assert_eq!(
        resolve_link("https://a.com/x/y", "z?q=1").as_deref(),
        Some("https://a.com/x/z?q=1")
    );
    assert_eq!(
        resolve_link("https://a.com/", "https://b.com/p").as_deref(),
        Some("https://b.com/p")
    );
    assert_eq!(resolve_link("https://a.com/", "#top"), None);
    assert_eq!(resolve_link("https://a.com/p", "https://a.com/p#x"), None);
    assert_eq!(
        resolve_link("https://a.com/p", "https://a.com/q#x").as_deref(),
        Some("https://a.com/q#x")
    );
    assert_eq!(resolve_link("https://a.com/", "javascript:void(0)"), None);
    assert_eq!(resolve_link("https://a.com/", "mailto:x@y"), None);
    assert_eq!(
        resolve_link("", "https://b.com/").as_deref(),
        Some("https://b.com/")
    );
    assert_eq!(resolve_link("", "/rel"), None);

    let mut b = Browser::new();
    let id = b.active_id();
    let g = b.navigate(id, "a.com", opts()).unwrap();
    b.finish(id, g.request, Ok(page("A")));
    let g = b.open_link(id, "/about", opts()).unwrap();
    assert_eq!(g.site.url, "https://a.com/about");
    assert_eq!(g.site.referer.as_deref(), Some("https://a.com/"));
    assert_eq!(b.active().address, "https://a.com/about");
    assert!(b.open_link(id, "#x", opts()).is_none());
}

#[test]
fn history_context_is_capped() {
    let mut b = Browser::new();
    let id = b.active_id();
    for i in 0..(MAX_ENTRIES + 5) {
        let g = b.navigate(id, &format!("s{i}.com"), opts()).unwrap();
        assert!(g.site.history.len() <= HISTORY_LIMIT);
        b.finish(id, g.request, Ok(page(&i.to_string())));
    }
    assert_eq!(b.active().entries().len(), MAX_ENTRIES);
    assert_eq!(b.active().entries()[0].title, "5");
}

#[test]
fn tab_icons_fall_back_sensibly() {
    let mut b = Browser::new();
    let id = b.active_id();
    assert_eq!(b.active().icon_svg(), NEW_TAB_ICON);
    assert_eq!(
        b.active().display_title(Language::Ru.messages()),
        "Новая вкладка"
    );
    let g = b.navigate(id, "noicon.example", opts()).unwrap();
    b.finish(id, g.request, Ok(page("N")));
    assert!(b.active().icon_svg().contains(">N</text>"));
    let g = b.navigate(id, "icon.example", opts()).unwrap();
    b.finish(
        id,
        g.request,
        Ok(
            "<head><link rel=icon href='data:image/svg+xml,%3Csvg%20id=%22fav%22/%3E'></head>"
                .into(),
        ),
    );
    assert_eq!(b.active().icon_svg(), "<svg id=\"fav\"/>");
    assert_eq!(b.active().title, "https://icon.example/");
}

#[test]
fn status_texts_translate() {
    let ru = Language::Ru.messages();
    assert_eq!(
        Status::SavedPdf {
            path: "/x.pdf".into()
        }
        .text(ru),
        "PDF сохранён: /x.pdf"
    );
    assert_eq!(Status::NothingToSave.text(ru), "Нечего сохранять");
    assert!(Status::FailedPdf { error: "e".into() }.is_error());
    assert!(!Status::PdfCanceled.is_error());
}
