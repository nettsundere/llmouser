//! Application state, the application delegate and every user action.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSImage, NSMenu,
    NSMenuItem, NSMenuItemValidation, NSPrintOperation, NSWorkspace,
};
use objc2_foundation::{NSData, NSNotification, NSObject, NSObjectProtocol, NSURL};

use llmouser_browser::{Language, TabId};

use super::about::AboutWindow;
use super::settings::SettingsWindow;
use super::util::{ns, obj_ptr, on_main};
use super::webview::WebContext;
use super::window::{self, TabWindow};
use super::{automation, menu, pdf};
use crate::session::{self, Session};

/// Dock-style icon (inset squircle) used for the dock and the About window.
pub const ICON_PNG: &[u8] = include_bytes!("../../../../assets/icon-mac.png");

pub struct State {
    pub mtm: MainThreadMarker,
    pub session: RefCell<Session>,
    pub windows: RefCell<Vec<Rc<TabWindow>>>,
    pub delegate: Retained<AppDelegate>,
    pub web: WebContext,
    pub settings: RefCell<Option<Rc<SettingsWindow>>>,
    pub about: RefCell<Option<Rc<AboutWindow>>>,
    pub context_menu: RefCell<Option<Retained<NSMenu>>>,
    pub pending_pdf: RefCell<Option<(TabId, PathBuf)>>,
    pub icon: Option<Retained<NSImage>>,
    /// Tab whose window was key most recently (settings/about windows may be key now).
    pub last_active: Cell<Option<TabId>>,
}

thread_local! {
    static STATE: RefCell<Option<Rc<State>>> = const { RefCell::new(None) };
}

pub fn state() -> Rc<State> {
    STATE.with(|s| s.borrow().clone().expect("app state initialised"))
}

pub fn app(mtm: MainThreadMarker) -> Retained<NSApplication> {
    NSApplication::sharedApplication(mtm)
}

pub fn run() {
    let mtm = MainThreadMarker::new().expect("must run on the main thread");
    let app = app(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let session = Session::new(session::sink(|event| {
        on_main(move || {
            let st = state();
            let changed = st.session.borrow_mut().on_event(event);
            if let Some(tab) = changed {
                window::render(tab);
            }
        })
    }));
    let delegate = AppDelegate::new(mtm);
    let icon = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(ICON_PNG));
    let state = Rc::new(State {
        mtm,
        session: RefCell::new(session),
        windows: RefCell::new(Vec::new()),
        delegate: delegate.clone(),
        web: WebContext::new(mtm),
        settings: RefCell::new(None),
        about: RefCell::new(None),
        context_menu: RefCell::new(None),
        pending_pdf: RefCell::new(None),
        icon,
        last_active: Cell::new(None),
    });
    STATE.with(|s| *s.borrow_mut() = Some(state));
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserAppDelegate"]
    #[ivars = ()]
    pub struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            launched();
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _sender: &NSApplication) -> bool {
            false
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _sender: &NSApplication, has_visible_windows: bool) -> bool {
            if !has_visible_windows {
                open_window(None);
            }
            true
        }
    }

    unsafe impl NSMenuItemValidation for AppDelegate {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            menu::validate(item)
        }
    }

    impl AppDelegate {
        #[unsafe(method(goBack:))]
        fn go_back(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                state().session.borrow_mut().back(w.tab);
                window::render(w.tab);
            }
        }

        #[unsafe(method(goForward:))]
        fn go_forward(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                state().session.borrow_mut().forward(w.tab);
                window::render(w.tab);
            }
        }

        #[unsafe(method(navigate:))]
        fn navigate(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                let text = w.address.stringValue().to_string();
                state().session.borrow_mut().go(w.tab, &text);
                window::render(w.tab);
                w.focus_page();
            }
        }

        #[unsafe(method(stopLoading:))]
        fn stop_loading(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                state().session.borrow_mut().stop(w.tab);
                window::render(w.tab);
            }
        }

        #[unsafe(method(reloadPage:))]
        fn reload_page(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                state().session.borrow_mut().reload(w.tab);
                window::render(w.tab);
            }
        }

        #[unsafe(method(focusAddress:))]
        fn focus_address(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                w.focus_address();
            }
        }

        #[unsafe(method(newTab:))]
        fn new_tab(&self, sender: Option<&AnyObject>) {
            let parent = window_for_sender(sender);
            open_window(parent);
        }

        #[unsafe(method(newWindowForTab:))]
        fn new_window_for_tab(&self, sender: Option<&AnyObject>) {
            let parent = window_for_sender(sender);
            open_window(parent);
        }

        #[unsafe(method(newWindow:))]
        fn new_window(&self, _sender: Option<&AnyObject>) {
            open_window(None);
        }

        #[unsafe(method(reopenClosedTab:))]
        fn reopen_closed_tab(&self, sender: Option<&AnyObject>) {
            let parent = window_for_sender(sender);
            let reopened = state().session.borrow_mut().browser.reopen_closed();
            if let Some(tab) = reopened {
                window::create(tab, parent.as_deref());
                window::render(tab);
            }
        }

        /// Close Tab: each tab is a window, so this closes the current one. Explicit
        /// target (not the responder chain) so it works even while the app is inactive.
        #[unsafe(method(closeTab:))]
        fn close_tab(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                w.window.performClose(None);
            }
        }

        #[unsafe(method(closeWindow:))]
        fn close_window(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                let group: Vec<_> = w
                    .window
                    .tabbedWindows()
                    .map(|a| a.to_vec())
                    .unwrap_or_else(|| vec![w.window.clone()]);
                for win in group {
                    win.performClose(None);
                }
            }
        }

        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            open_settings();
        }

        #[unsafe(method(savePdf:))]
        fn save_pdf(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                pdf::save_pdf(&w);
            }
        }

        #[unsafe(method(saveHtml:))]
        fn save_html(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                pdf::save_html(&w);
            }
        }

        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn print_did_run(&self, _op: &NSPrintOperation, success: bool, _ctx: *mut std::ffi::c_void) {
            pdf::print_finished(success);
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            open_about();
        }

        #[unsafe(method(openHomepage:))]
        fn open_homepage(&self, _sender: Option<&AnyObject>) {
            if let Some(url) = NSURL::URLWithString(&ns(llmouser_browser::APP_HOMEPAGE)) {
                NSWorkspace::sharedWorkspace().openURL(&url);
            }
        }

        #[unsafe(method(resetZoom:))]
        fn reset_zoom(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                w.zoom_by(None);
            }
        }

        #[unsafe(method(zoomIn:))]
        fn zoom_in(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                w.zoom_by(Some(1.1));
            }
        }

        #[unsafe(method(zoomOut:))]
        fn zoom_out(&self, sender: Option<&AnyObject>) {
            if let Some(w) = window_for_sender(sender) {
                w.zoom_by(Some(1.0 / 1.1));
            }
        }

        #[unsafe(method(providerChanged:))]
        fn provider_changed(&self, _sender: Option<&AnyObject>) {
            if let Some(s) = state().settings.borrow().clone() {
                s.provider_changed();
            }
        }

        #[unsafe(method(languageChanged:))]
        fn language_changed(&self, _sender: Option<&AnyObject>) {
            let picked = state().settings.borrow().clone().map(|s| s.picked_language());
            if let Some(language) = picked {
                set_language(language);
            }
        }

        #[unsafe(method(saveSettings:))]
        fn save_settings(&self, _sender: Option<&AnyObject>) {
            let settings = state().settings.borrow().clone();
            if let Some(s) = settings {
                let form = s.form();
                let before = state().session.borrow().language();
                let result = state().session.borrow_mut().save_settings(&form);
                match result {
                    Ok(_) => {
                        if form.language != before {
                            retranslate();
                        }
                        // The API key may have appeared: empty tabs show the start page.
                        for w in state().windows.borrow().iter() {
                            window::refresh_empty(w);
                        }
                        s.close();
                    }
                    Err(problem) => s.show_validation(&problem),
                }
            }
        }

        #[unsafe(method(closeSettings:))]
        fn close_settings(&self, _sender: Option<&AnyObject>) {
            if let Some(s) = state().settings.borrow().clone() {
                s.close();
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }

    pub fn as_target(&self) -> &AnyObject {
        self
    }
}

fn launched() {
    let st = state();
    let app = app(st.mtm);
    if let Some(icon) = &st.icon {
        unsafe { app.setApplicationIconImage(Some(icon)) };
    }
    st.web.install_network_block(st.mtm);
    menu::install(st.session.borrow().language());
    open_window(None);
    if let Some(port) = std::env::var(llmouser_browser::env::E2E_PORT)
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        if let Err(e) = automation::start(port) {
            eprintln!("automation server failed on port {port}: {e}");
        }
    }
    app.activate();
}

/// Open a new tab: as a native window tab next to `parent`, or a separate window.
pub fn open_window(parent: Option<Rc<TabWindow>>) -> Rc<TabWindow> {
    let tab = state().session.borrow_mut().browser.new_tab();
    let w = window::create(tab, parent.as_deref());
    window::render(tab);
    w
}

/// The tab window an action refers to: the sender's own window when the sender
/// is one of its controls, else the key window, else the most recent one.
pub fn window_for_sender(sender: Option<&AnyObject>) -> Option<Rc<TabWindow>> {
    let st = state();
    let windows = st.windows.borrow();
    if let Some(sender) = sender {
        let p: *const AnyObject = sender;
        if let Some(w) = windows.iter().find(|w| w.owns(p)) {
            return Some(w.clone());
        }
        // A window (e.g. the sender of newWindowForTab:) or one of its views.
        if let Some(w) = windows.iter().find(|w| obj_ptr(&*w.window) == p) {
            return Some(w.clone());
        }
    }
    current_window_in(&windows)
}

pub fn current_window() -> Option<Rc<TabWindow>> {
    let st = state();
    let windows = st.windows.borrow();
    current_window_in(&windows)
}

/// Key-window changes are delivered asynchronously, so the tab we last made
/// key (or that last became key) wins over what AppKit reports right now.
fn current_window_in(windows: &[Rc<TabWindow>]) -> Option<Rc<TabWindow>> {
    let st = state();
    if let Some(tab) = st.last_active.get() {
        if let Some(w) = windows.iter().find(|w| w.tab == tab) {
            return Some(w.clone());
        }
    }
    if let Some(key) = app(st.mtm).keyWindow() {
        if let Some(w) = windows.iter().find(|w| std::ptr::eq(&*w.window, &*key)) {
            return Some(w.clone());
        }
    }
    windows.first().cloned()
}

/// Remember which tab is current (called when we make a window key ourselves).
pub fn set_current(tab: TabId) {
    on_window_key(tab);
}

pub fn on_window_key(tab: TabId) {
    let st = state();
    st.last_active.set(Some(tab));
    st.session.borrow_mut().browser.activate(tab);
}

pub fn on_window_closed(tab: TabId) {
    let st = state();
    let outcome = st.session.borrow_mut().close_tab(tab);
    if st.last_active.get() == Some(tab) {
        // The neighbour AppKit selects becomes key soon; until then, follow the model.
        st.last_active.set(outcome.activated);
    }
    // Drop our reference after AppKit has finished closing the window.
    on_main(move || {
        let st = state();
        st.windows.borrow_mut().retain(|w| w.tab != tab);
    });
}

pub fn on_address_typed(tab: TabId, text: &str) {
    state().session.borrow_mut().browser.set_address(tab, text);
}

pub fn open_settings() {
    let st = state();
    let existing = st.settings.borrow().clone();
    let window = match existing {
        Some(s) => s,
        None => {
            let s = SettingsWindow::new(st.mtm);
            *st.settings.borrow_mut() = Some(s.clone());
            s
        }
    };
    window.show();
}

pub fn open_about() {
    let st = state();
    let existing = st.about.borrow().clone();
    let about = match existing {
        Some(a) => a,
        None => {
            let a = AboutWindow::new(st.mtm);
            *st.about.borrow_mut() = Some(a.clone());
            a
        }
    };
    about.show();
}

/// The instant language switch from the settings dialog.
pub fn set_language(language: Language) {
    let st = state();
    let changed = st.session.borrow().language() != language;
    if !changed {
        return;
    }
    if let Err(e) = st.session.borrow_mut().set_language(language) {
        eprintln!("could not save language: {e}");
    }
    retranslate();
}

/// Retranslate everything on screen after a language change.
pub fn retranslate() {
    let st = state();
    menu::install(st.session.borrow().language());
    window::render_all();
    let settings = st.settings.borrow().clone();
    if let Some(s) = settings {
        s.retranslate();
    }
    let about = st.about.borrow().clone();
    if let Some(a) = about {
        a.retranslate();
    }
}
