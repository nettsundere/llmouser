//! WebKitGTK views: ephemeral network session, a content filter that blocks
//! every real network request, and the navigation policy that turns page
//! navigations into generations.

use std::path::PathBuf;

use gtk::gio;
use gtk::glib;
use webkit6 as webkit;
use webkit6::prelude::*;

use llmouser_browser::page::CHROME_SCHEME;
use llmouser_browser::TabId;

use super::app::{self, state};
use super::menu;
use super::window::render;

/// Content-blocker rules (same JSON dialect as WebKit on macOS).
pub const NETWORK_BLOCK_RULES: &str = r#"[
  {"trigger":{"url-filter":"^https?://.*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^wss?://.*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^ftp://.*"},"action":{"type":"block"}}
]"#;

pub struct WebContext {
    pub network: webkit::NetworkSession,
    pub content: webkit::UserContentManager,
    pub settings: webkit::Settings,
}

impl WebContext {
    pub fn new(filter_dir: Option<PathBuf>) -> WebContext {
        let network = webkit::NetworkSession::new_ephemeral();
        let content = webkit::UserContentManager::new();
        let settings = webkit::Settings::new();
        settings.set_javascript_can_open_windows_automatically(false);
        settings.set_enable_developer_extras(false);
        let dir = filter_dir
            .unwrap_or_else(std::env::temp_dir)
            .join("filters");
        let _ = std::fs::create_dir_all(&dir);
        let store = webkit::UserContentFilterStore::new(&dir.display().to_string());
        let manager = content.clone();
        store.save(
            "llmouser-network-block",
            &glib::Bytes::from_static(NETWORK_BLOCK_RULES.as_bytes()),
            None::<&gio::Cancellable>,
            move |result| match result {
                Ok(filter) => manager.add_filter(&filter),
                Err(e) => eprintln!("content filter failed: {e}"),
            },
        );
        WebContext {
            network,
            content,
            settings,
        }
    }
}

pub fn create(ctx: &WebContext, tab: TabId) -> webkit::WebView {
    let webview = webkit::WebView::builder()
        .network_session(&ctx.network)
        .user_content_manager(&ctx.content)
        .settings(&ctx.settings)
        .hexpand(true)
        .vexpand(true)
        .build();

    webview.connect_decide_policy(move |_wv, decision, kind| match kind {
        webkit::PolicyDecisionType::NavigationAction
        | webkit::PolicyDecisionType::NewWindowAction => {
            let uri = decision
                .downcast_ref::<webkit::NavigationPolicyDecision>()
                .and_then(|d| d.navigation_action())
                .and_then(|a| a.request())
                .and_then(|r| r.uri())
                .map(|u| u.to_string())
                .unwrap_or_default();
            let new_window = kind == webkit::PolicyDecisionType::NewWindowAction;
            if decide(tab, &uri, new_window) {
                decision.use_();
            } else {
                decision.ignore();
            }
            true
        }
        _ => false,
    });

    webview.connect_context_menu(move |_wv, context_menu, _hit| {
        context_menu.remove_all();
        for item in menu::context_items() {
            context_menu.append(&item);
        }
        false
    });
    webview
}

/// Whether the navigation may proceed. Our own document loads and in-page
/// anchors do; everything else is regenerated through the LLM.
fn decide(tab: TabId, url: &str, new_window: bool) -> bool {
    let w = app::main_window();
    let Some(entry) = w.entry_for(tab) else {
        return false;
    };
    if let Some(rest) = url.strip_prefix(&format!("{CHROME_SCHEME}://")) {
        chrome_action(tab, rest.trim_end_matches('/'));
        return false;
    }
    if url.is_empty() || url == "about:blank" {
        return true;
    }
    if !new_window {
        let expected = entry.expecting.borrow().clone();
        if expected.map(|e| same_url(&e, url)).unwrap_or(false) {
            entry.expecting.take();
            return true;
        }
        let current = state()
            .session
            .borrow()
            .browser
            .tab(tab)
            .map(|t| t.url.clone())
            .unwrap_or_default();
        if same_document(&current, url) {
            return true;
        }
    }
    state().session.borrow_mut().open_link(tab, url);
    render(tab);
    false
}

pub fn chrome_action(tab: TabId, action: &str) {
    match action {
        "retry" => {
            state().session.borrow_mut().reload(tab);
            render(tab);
        }
        "settings" => app::open_settings(),
        _ => {}
    }
}

pub fn same_url(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

pub fn same_document(current: &str, target: &str) -> bool {
    let (Ok(mut a), Ok(mut b)) = (url::Url::parse(current), url::Url::parse(target)) else {
        return false;
    };
    if b.fragment().is_none() {
        return false;
    }
    a.set_fragment(None);
    b.set_fragment(None);
    a == b
}

/// Evaluate JavaScript in a view; the JSON-encoded result arrives in `done`.
pub fn eval(
    webview: &webkit::WebView,
    js: &str,
    done: impl FnOnce(Result<String, String>) + 'static,
) {
    webview.evaluate_javascript(js, None, None, None::<&gio::Cancellable>, move |result| {
        done(match result {
            Ok(value) => Ok(value.to_str().to_string()),
            Err(e) => Err(e.to_string()),
        })
    });
}
