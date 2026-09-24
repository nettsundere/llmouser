//! WKWebView setup: the shared configuration with a network content blocker,
//! the webview subclass that owns the context menu, and the navigation / UI
//! delegates that turn every navigation the page attempts into a generation.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAlert, NSEvent, NSMenu};
use objc2_foundation::{NSError, NSObject, NSRect, NSString};
use objc2_web_kit::{
    WKContentRuleList, WKContentRuleListStore, WKFrameInfo, WKNavigationAction,
    WKNavigationActionPolicy, WKNavigationDelegate, WKUIDelegate, WKUserContentController,
    WKWebView, WKWebViewConfiguration, WKWebsiteDataStore, WKWindowFeatures,
};

use llmouser_browser::page::CHROME_SCHEME;
use llmouser_browser::TabId;

use super::app::{self, state};
use super::util::ns;
use super::{menu, window};

/// Content-blocker rules: no real network from any page, in any frame.
/// WebKit's rule regexes have no alternation, hence one rule per scheme.
pub const NETWORK_BLOCK_RULES: &str = r#"[
  {"trigger":{"url-filter":"^https?://.*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^wss?://.*"},"action":{"type":"block"}},
  {"trigger":{"url-filter":"^ftp://.*"},"action":{"type":"block"}}
]"#;

pub struct WebContext {
    pub config: Retained<WKWebViewConfiguration>,
    pub controller: Retained<WKUserContentController>,
}

impl WebContext {
    pub fn new(mtm: MainThreadMarker) -> WebContext {
        unsafe {
            let config = WKWebViewConfiguration::new(mtm);
            let controller = WKUserContentController::new(mtm);
            config.setUserContentController(&controller);
            config.setWebsiteDataStore(&WKWebsiteDataStore::nonPersistentDataStore(mtm));
            config
                .preferences()
                .setJavaScriptCanOpenWindowsAutomatically(false);
            WebContext { config, controller }
        }
    }

    /// Compile and attach the content blocker; applies to every webview that
    /// shares the user content controller, including ones created before.
    pub fn install_network_block(&self, mtm: MainThreadMarker) {
        let Some(store) = (unsafe { WKContentRuleListStore::defaultStore(mtm) }) else {
            eprintln!("content rule list store unavailable; relying on CSP only");
            return;
        };
        let controller = self.controller.clone();
        let block = RcBlock::new(move |list: *mut WKContentRuleList, error: *mut NSError| {
            if list.is_null() {
                let message =
                    unsafe { error.as_ref() }.map(|e| e.localizedDescription().to_string());
                eprintln!("content rule list failed: {}", message.unwrap_or_default());
                return;
            }
            unsafe { controller.addContentRuleList(&*list) };
        });
        unsafe {
            store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(
                Some(&ns("llmouser-network-block")),
                Some(&ns(NETWORK_BLOCK_RULES)),
                Some(&block),
            );
        }
    }
}

// ---- The webview: owns the context menu ---------------------------------

define_class!(
    #[unsafe(super(WKWebView))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserWebView"]
    #[ivars = TabId]
    pub struct LLMWebView;

    impl LLMWebView {
        #[unsafe(method(willOpenMenu:withEvent:))]
        fn will_open_menu(&self, menu: &NSMenu, _event: &NSEvent) {
            menu::populate_context_menu(menu, *self.ivars());
        }
    }
);

impl LLMWebView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        config: &WKWebViewConfiguration,
        tab: TabId,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(tab);
        unsafe { msg_send![super(this), initWithFrame: frame, configuration: config] }
    }
}

// ---- Navigation policy --------------------------------------------------

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserNavigationDelegate"]
    #[ivars = TabId]
    pub struct NavigationDelegate;

    unsafe impl NSObjectProtocol for NavigationDelegate {}

    unsafe impl WKNavigationDelegate for NavigationDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        unsafe fn decide_policy(
            &self,
            _web_view: &WKWebView,
            action: &WKNavigationAction,
            handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let policy = decide(*self.ivars(), action);
            handler.call((policy,));
        }
    }
);

impl NavigationDelegate {
    pub fn new(mtm: MainThreadMarker, tab: TabId) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(tab);
        unsafe { msg_send![super(this), init] }
    }
}

/// Every navigation the page attempts lands here: our own document loads are
/// allowed, in-page anchors are allowed, chrome actions are executed, and
/// anything else is cancelled and regenerated through the LLM instead.
fn decide(tab: TabId, action: &WKNavigationAction) -> WKNavigationActionPolicy {
    let url = unsafe { action.request().URL() }
        .and_then(|u| u.absoluteString())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let Some(win) = window::find(tab) else {
        return WKNavigationActionPolicy::Cancel;
    };

    if let Some(rest) = url.strip_prefix(&format!("{CHROME_SCHEME}://")) {
        chrome_action(tab, rest.trim_end_matches('/'));
        return WKNavigationActionPolicy::Cancel;
    }
    if url.is_empty() || url == "about:blank" {
        return WKNavigationActionPolicy::Allow;
    }
    let expected = win.expecting.borrow().clone();
    if expected.map(|e| same_url(&e, &url)).unwrap_or(false) {
        win.expecting.take();
        return WKNavigationActionPolicy::Allow;
    }
    let is_main = unsafe { action.targetFrame() }
        .map(|f| unsafe { f.isMainFrame() })
        .unwrap_or(true);
    if !is_main {
        return WKNavigationActionPolicy::Cancel;
    }
    let current = state()
        .session
        .borrow()
        .browser
        .tab(tab)
        .map(|t| t.url.clone())
        .unwrap_or_default();
    if same_document(&current, &url) {
        return WKNavigationActionPolicy::Allow;
    }
    state().session.borrow_mut().open_link(tab, &url);
    window::render(tab);
    WKNavigationActionPolicy::Cancel
}

/// `llmouser://retry` and `llmouser://settings` from the chrome's own pages.
pub fn chrome_action(tab: TabId, action: &str) {
    match action {
        "retry" => {
            state().session.borrow_mut().reload(tab);
            window::render(tab);
        }
        "settings" => app::open_settings(),
        _ => {}
    }
}

/// URL equality as WebKit reports it (`https://a.com` and `https://a.com/` are one URL).
pub fn same_url(a: &str, b: &str) -> bool {
    match (url::Url::parse(a), url::Url::parse(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// A fragment-only change of the current page: WebKit scrolls, no new document.
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

// ---- UI delegate: popups, alerts ----------------------------------------

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserUiDelegate"]
    #[ivars = TabId]
    pub struct UiDelegate;

    unsafe impl NSObjectProtocol for UiDelegate {}

    unsafe impl WKUIDelegate for UiDelegate {
        #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        unsafe fn create_web_view(
            &self,
            _web_view: &WKWebView,
            _configuration: &WKWebViewConfiguration,
            action: &WKNavigationAction,
            _features: &WKWindowFeatures,
        ) -> Option<Retained<WKWebView>> {
            // target=_blank / window.open: never a real window; generate in this tab.
            let tab = *self.ivars();
            if let Some(url) = action.request().URL().and_then(|u| u.absoluteString()) {
                state()
                    .session
                    .borrow_mut()
                    .open_link(tab, &url.to_string());
                window::render(tab);
            }
            None
        }

        #[unsafe(method(webView:runJavaScriptAlertPanelWithMessage:initiatedByFrame:completionHandler:))]
        unsafe fn run_alert(
            &self,
            _web_view: &WKWebView,
            message: &NSString,
            _frame: &WKFrameInfo,
            handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let alert = NSAlert::new(state().mtm);
            alert.setMessageText(message);
            alert.runModal();
            handler.call(());
        }
    }
);

impl UiDelegate {
    pub fn new(mtm: MainThreadMarker, tab: TabId) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(tab);
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(test)]
mod tests {
    use super::same_document;

    #[test]
    fn fragment_navigations_stay_in_document() {
        assert!(same_document("https://a.com/p", "https://a.com/p#x"));
        assert!(same_document("https://a.com/p#y", "https://a.com/p#x"));
        assert!(!same_document("https://a.com/p", "https://a.com/q#x"));
        assert!(!same_document("https://a.com/p", "https://a.com/p"));
        assert!(!same_document("", "https://a.com/p#x"));
        assert!(super::same_url("https://a.com", "https://a.com/"));
        assert!(!super::same_url("https://a.com/x", "https://a.com/"));
    }
}
