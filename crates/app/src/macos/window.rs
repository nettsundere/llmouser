//! One native window per tab. Windows share a tabbing identifier, so AppKit
//! groups them in its own tab bar (Cmd+T, Cmd+W, Cmd+Shift+], drag-to-reorder
//! all come from the system). Each window has a unified toolbar (back, forward,
//! address, go/stop, settings), the page webview and a status line.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSControlTextEditingDelegate, NSFont, NSLayoutConstraint,
    NSLineBreakMode, NSTextField, NSTextFieldBezelStyle, NSTextFieldDelegate, NSToolbar,
    NSToolbarDelegate, NSToolbarDisplayMode, NSToolbarItem, NSToolbarItemIdentifier, NSWindow,
    NSWindowDelegate, NSWindowOrderingMode, NSWindowStyleMask, NSWindowTabbingMode,
    NSWindowToolbarStyle,
};
use objc2_foundation::{NSArray, NSNotification, NSObject, NSRectEdge, NSString, NSURL};

use llmouser_browser::TabId;

use super::app::{self, state};
use super::util::{ns, obj_ptr, rect, size};
use super::webview::{LLMWebView, NavigationDelegate, UiDelegate};

const ITEM_BACK: &str = "back";
const ITEM_FORWARD: &str = "forward";
const ITEM_ADDRESS: &str = "address";
const ITEM_GO: &str = "go";
const ITEM_SETTINGS: &str = "settings";
/// Height of the status bar at the bottom of every window.
const STATUS_HEIGHT: f64 = 22.0;

pub struct TabWindow {
    pub tab: TabId,
    pub window: Retained<NSWindow>,
    pub webview: Retained<LLMWebView>,
    pub address: Retained<NSTextField>,
    pub status: Retained<NSTextField>,
    pub back_item: Retained<NSToolbarItem>,
    pub forward_item: Retained<NSToolbarItem>,
    pub go_item: Retained<NSToolbarItem>,
    pub settings_item: Retained<NSToolbarItem>,
    pub address_item: Retained<NSToolbarItem>,
    _toolbar: Retained<NSToolbar>,
    _toolbar_delegate: Retained<ToolbarDelegate>,
    _window_delegate: Retained<WindowDelegate>,
    _nav_delegate: Retained<NavigationDelegate>,
    _ui_delegate: Retained<UiDelegate>,
    /// Content sequence last loaded into the webview.
    pub loaded_seq: Cell<u64>,
    /// URL of the document we asked the webview to load, so the navigation
    /// policy can tell our own loads from the page's navigations.
    pub expecting: RefCell<Option<String>>,
    /// Whether the go item currently acts as "stop".
    pub stopping: Cell<bool>,
    pub zoom: Cell<f64>,
}

impl TabWindow {
    /// Whether an action sender belongs to this window.
    pub fn owns(&self, p: *const AnyObject) -> bool {
        let candidates: [*const AnyObject; 8] = [
            obj_ptr(&*self.window),
            obj_ptr(&*self.webview),
            obj_ptr(&*self.address),
            obj_ptr(&*self.back_item),
            obj_ptr(&*self.forward_item),
            obj_ptr(&*self.go_item),
            obj_ptr(&*self.settings_item),
            obj_ptr(&*self.address_item),
        ];
        candidates.contains(&p)
    }

    pub fn focus_address(&self) {
        self.window.makeFirstResponder(Some(&self.address));
    }

    pub fn focus_page(&self) {
        self.window.makeFirstResponder(Some(&self.webview));
    }

    pub fn zoom_by(&self, factor: Option<f64>) {
        let zoom = match factor {
            Some(f) => (self.zoom.get() * f).clamp(0.25, 5.0),
            None => 1.0,
        };
        self.zoom.set(zoom);
        unsafe { self.webview.setPageZoom(zoom) };
    }

    pub fn is_loading(&self) -> bool {
        self.stopping.get()
    }
}

/// Create the window for `tab`, attached as a native tab of `parent` when given.
pub fn create(tab: TabId, parent: Option<&TabWindow>) -> Rc<TabWindow> {
    let st = state();
    let mtm = st.mtm;
    let m = st.session.borrow().messages();
    let delegate = st.delegate.as_target();

    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            rect(0.0, 0.0, 1200.0, 800.0),
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&ns(m.new_tab));
    window.setMinSize(size(480.0, 320.0));
    window.setTabbingMode(NSWindowTabbingMode::Preferred);
    window.setTabbingIdentifier(&ns("llmouser-browser"));
    window.setToolbarStyle(NSWindowToolbarStyle::Unified);

    // Toolbar items.
    let back_item = image_item(
        mtm,
        ITEM_BACK,
        "chevron.left",
        m.back,
        delegate,
        sel!(goBack:),
    );
    let forward_item = image_item(
        mtm,
        ITEM_FORWARD,
        "chevron.right",
        m.forward,
        delegate,
        sel!(goForward:),
    );
    let go_item = image_item(
        mtm,
        ITEM_GO,
        "arrow.right.circle",
        m.go,
        delegate,
        sel!(navigate:),
    );
    let settings_item = image_item(
        mtm,
        ITEM_SETTINGS,
        "gearshape",
        m.settings,
        delegate,
        sel!(openSettings:),
    );

    let address = NSTextField::textFieldWithString(&ns(""), mtm);
    address.setPlaceholderString(Some(&ns(m.address_placeholder)));
    address.setBezelStyle(NSTextFieldBezelStyle::RoundedBezel);
    address.setUsesSingleLineMode(true);
    address.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    address.setToolTip(Some(&ns(m.address_bar)));
    unsafe {
        address.setTarget(Some(delegate));
        address.setAction(Some(sel!(navigate:)));
    }
    let address_item =
        NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(ITEM_ADDRESS));
    address_item.setView(Some(&address));
    address_item.setLabel(&ns(m.address_bar));
    #[allow(deprecated)]
    {
        address_item.setMinSize(size(220.0, 24.0));
        address_item.setMaxSize(size(3000.0, 24.0));
    }

    let toolbar_delegate = ToolbarDelegate::new(
        mtm,
        vec![
            back_item.clone(),
            forward_item.clone(),
            address_item.clone(),
            go_item.clone(),
            settings_item.clone(),
        ],
    );
    let toolbar = NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), &ns("llmouser-toolbar"));
    toolbar.setDelegate(Some(ProtocolObject::from_ref(&*toolbar_delegate)));
    toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
    toolbar.setAllowsUserCustomization(false);
    window.setToolbar(Some(&toolbar));

    // Content: webview above a status line.
    let content = window.contentView().expect("window content view");
    let webview = LLMWebView::new(mtm, content.bounds(), &st.web.config, tab);
    let nav_delegate = NavigationDelegate::new(mtm, tab);
    let ui_delegate = UiDelegate::new(mtm, tab);
    unsafe {
        webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*nav_delegate)));
        webview.setUIDelegate(Some(ProtocolObject::from_ref(&*ui_delegate)));
    }
    // Status bar: the window's native bottom content border (as in Finder and
    // Mail), which draws its own texture and separator, with the label centred
    // vertically inside it.
    window.setAutorecalculatesContentBorderThickness_forEdge(false, NSRectEdge::NSMinYEdge);
    window.setContentBorderThickness_forEdge(STATUS_HEIGHT, NSRectEdge::NSMinYEdge);
    let status = NSTextField::labelWithString(&ns(m.ready), mtm);
    status.setFont(Some(&NSFont::systemFontOfSize(
        NSFont::smallSystemFontSize(),
    )));
    status.setTextColor(Some(&NSColor::secondaryLabelColor()));
    status.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
    status.setUsesSingleLineMode(true);

    webview.setTranslatesAutoresizingMaskIntoConstraints(false);
    status.setTranslatesAutoresizingMaskIntoConstraints(false);
    content.addSubview(&webview);
    content.addSubview(&status);
    let constraints = [
        webview
            .topAnchor()
            .constraintEqualToAnchor(&content.topAnchor()),
        webview
            .leadingAnchor()
            .constraintEqualToAnchor(&content.leadingAnchor()),
        webview
            .trailingAnchor()
            .constraintEqualToAnchor(&content.trailingAnchor()),
        webview
            .bottomAnchor()
            .constraintEqualToAnchor_constant(&content.bottomAnchor(), -STATUS_HEIGHT),
        status
            .leadingAnchor()
            .constraintEqualToAnchor_constant(&content.leadingAnchor(), 20.0),
        status
            .trailingAnchor()
            .constraintLessThanOrEqualToAnchor_constant(&content.trailingAnchor(), -20.0),
        status
            .centerYAnchor()
            .constraintEqualToAnchor_constant(&content.bottomAnchor(), -STATUS_HEIGHT / 2.0),
    ];
    NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&constraints));

    let window_delegate = WindowDelegate::new(mtm, tab);
    window.setDelegate(Some(ProtocolObject::from_ref(&*window_delegate)));
    unsafe { address.setDelegate(Some(ProtocolObject::from_ref(&*window_delegate))) };

    let tw = Rc::new(TabWindow {
        tab,
        window: window.clone(),
        webview,
        address,
        status,
        back_item,
        forward_item,
        go_item,
        settings_item,
        address_item,
        _toolbar: toolbar,
        _toolbar_delegate: toolbar_delegate,
        _window_delegate: window_delegate,
        _nav_delegate: nav_delegate,
        _ui_delegate: ui_delegate,
        loaded_seq: Cell::new(u64::MAX),
        expecting: RefCell::new(None),
        stopping: Cell::new(false),
        zoom: Cell::new(1.0),
    });
    st.windows.borrow_mut().push(tw.clone());

    match parent {
        Some(p) => p
            .window
            .addTabbedWindow_ordered(&window, NSWindowOrderingMode::Above),
        None => window.center(),
    }
    window.makeKeyAndOrderFront(None);
    app::set_current(tab);
    tw.focus_address();
    tw
}

fn image_item(
    mtm: MainThreadMarker,
    id: &str,
    symbol: &str,
    label: &str,
    target: &AnyObject,
    action: objc2::runtime::Sel,
) -> Retained<NSToolbarItem> {
    let item = NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), &ns(id));
    if let Some(image) = super::util::symbol(symbol, label) {
        item.setImage(Some(&image));
    }
    item.setBordered(true);
    item.setLabel(&ns(label));
    item.setToolTip(Some(&ns(label)));
    item.setAutovalidates(false);
    unsafe {
        item.setTarget(Some(target));
        item.setAction(Some(action));
    }
    item
}

pub fn find(tab: TabId) -> Option<Rc<TabWindow>> {
    state()
        .windows
        .borrow()
        .iter()
        .find(|w| w.tab == tab)
        .cloned()
}

/// Update every widget of a tab's window from the browser model.
pub fn render(tab: TabId) {
    let Some(w) = find(tab) else { return };
    let st = state();
    let snapshot = {
        let s = st.session.borrow();
        let Some(t) = s.browser.tab(tab) else { return };
        let m = s.messages();
        (
            t.display_title(m),
            t.address.clone(),
            t.can_back(),
            t.can_forward(),
            t.is_loading(),
            t.status.text(m),
            t.status.is_error(),
            t.content_seq,
            s.document_for(tab, false),
            m,
        )
    };
    let (title, address, can_back, can_forward, loading, status, is_error, seq, document, m) =
        snapshot;

    w.window.setTitle(&ns(&title));
    if w.address.stringValue().to_string() != address {
        w.address.setStringValue(&ns(&address));
    }
    w.address
        .setPlaceholderString(Some(&ns(m.address_placeholder)));
    w.address.setToolTip(Some(&ns(m.address_bar)));
    w.back_item.setEnabled(can_back);
    w.forward_item.setEnabled(can_forward);
    set_tooltip(&w.back_item, m.back);
    set_tooltip(&w.forward_item, m.forward);
    set_tooltip(&w.settings_item, m.settings);

    if loading != w.stopping.get() {
        w.stopping.set(loading);
        let (symbol, action) = if loading {
            ("xmark.circle", sel!(stopLoading:))
        } else {
            ("arrow.right.circle", sel!(navigate:))
        };
        if let Some(image) = super::util::symbol(symbol, if loading { m.stop } else { m.go }) {
            w.go_item.setImage(Some(&image));
        }
        unsafe { w.go_item.setAction(Some(action)) };
    }
    set_tooltip(&w.go_item, if loading { m.stop } else { m.go });

    w.status.setStringValue(&ns(&status));
    let color = if is_error {
        NSColor::systemRedColor()
    } else {
        NSColor::secondaryLabelColor()
    };
    w.status.setTextColor(Some(&color));

    if w.loaded_seq.get() != seq {
        w.loaded_seq.set(seq);
        if let Some((html, base)) = document {
            load_document(&w, &html, base.as_deref());
        }
    }
}

/// Empty tabs show the start page, which depends on whether a key is set.
pub fn refresh_empty(w: &TabWindow) {
    let st = state();
    let doc = {
        let s = st.session.borrow();
        match s.browser.tab(w.tab) {
            Some(t) if matches!(t.content, llmouser_browser::Content::Empty) => {
                s.document_for(w.tab, false)
            }
            _ => None,
        }
    };
    if let Some((html, base)) = doc {
        load_document(w, &html, base.as_deref());
    }
}

fn load_document(w: &TabWindow, html: &str, base: Option<&str>) {
    let base_url = base.and_then(|b| NSURL::URLWithString(&ns(b)));
    *w.expecting.borrow_mut() = Some(
        base.map(str::to_string)
            .unwrap_or_else(|| "about:blank".into()),
    );
    unsafe {
        w.webview
            .loadHTMLString_baseURL(&ns(html), base_url.as_deref())
    };
}

fn set_tooltip(item: &NSToolbarItem, text: &str) {
    item.setToolTip(Some(&ns(text)));
    item.setLabel(&ns(text));
}

pub fn render_all() {
    let tabs: Vec<TabId> = state().windows.borrow().iter().map(|w| w.tab).collect();
    for tab in tabs {
        // Chrome pages (start/error) are translated too: force a reload.
        if let Some(w) = find(tab) {
            w.loaded_seq.set(u64::MAX);
        }
        render(tab);
    }
}

// ---- Toolbar delegate ---------------------------------------------------

pub struct ToolbarIvars {
    items: Vec<Retained<NSToolbarItem>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserToolbarDelegate"]
    #[ivars = ToolbarIvars]
    pub struct ToolbarDelegate;

    unsafe impl NSObjectProtocol for ToolbarDelegate {}

    unsafe impl NSToolbarDelegate for ToolbarDelegate {
        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        fn item_for_identifier(
            &self,
            _toolbar: &NSToolbar,
            identifier: &NSToolbarItemIdentifier,
            _flag: bool,
        ) -> Option<Retained<NSToolbarItem>> {
            let wanted = identifier.to_string();
            self.ivars()
                .items
                .iter()
                .find(|i| i.itemIdentifier().to_string() == wanted)
                .cloned()
        }

        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn default_identifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.identifiers()
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn allowed_identifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.identifiers()
        }
    }
);

impl ToolbarDelegate {
    fn new(mtm: MainThreadMarker, items: Vec<Retained<NSToolbarItem>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ToolbarIvars { items });
        unsafe { msg_send![super(this), init] }
    }

    fn identifiers(&self) -> Retained<NSArray<NSString>> {
        let ids: Vec<Retained<NSString>> = self
            .ivars()
            .items
            .iter()
            .map(|i| i.itemIdentifier())
            .collect();
        NSArray::from_retained_slice(&ids)
    }
}

// ---- Window + address field delegate ------------------------------------

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserWindowDelegate"]
    #[ivars = TabId]
    pub struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            app::on_window_closed(*self.ivars());
        }

        #[unsafe(method(windowDidBecomeKey:))]
        fn window_did_become_key(&self, _notification: &NSNotification) {
            app::on_window_key(*self.ivars());
        }
    }

    unsafe impl NSControlTextEditingDelegate for WindowDelegate {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            let tab = *self.ivars();
            if let Some(w) = find(tab) {
                app::on_address_typed(tab, &w.address.stringValue().to_string());
            }
        }
    }

    unsafe impl NSTextFieldDelegate for WindowDelegate {}
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, tab: TabId) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(tab);
        unsafe { msg_send![super(this), init] }
    }
}
