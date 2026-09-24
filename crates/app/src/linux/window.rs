//! The browser window: libadwaita header bar (back, forward, address, go/stop,
//! settings), a menu bar, an `AdwTabBar` over an `AdwTabView` of WebKit views,
//! and a status line.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use webkit6 as webkit;
use webkit6::prelude::*;

use llmouser_browser::TabId;

use super::app::{self, state};
use super::{menu, webview};

pub struct TabEntry {
    pub tab: TabId,
    pub page: adw::TabPage,
    pub webview: webkit::WebView,
    pub loaded_seq: Cell<u64>,
    pub expecting: RefCell<Option<String>>,
    pub zoom: Cell<f64>,
    pub icon_kind: Cell<&'static str>,
}

pub struct MainWindow {
    pub window: adw::ApplicationWindow,
    pub tab_view: adw::TabView,
    _tab_bar: adw::TabBar,
    pub menubar: gtk::PopoverMenuBar,
    pub address: gtk::Entry,
    pub back: gtk::Button,
    pub forward: gtk::Button,
    pub go: gtk::Button,
    pub settings_button: gtk::Button,
    pub new_tab_button: gtk::Button,
    pub status: gtk::Label,
    pub tabs: RefCell<Vec<Rc<TabEntry>>>,
    /// Set while we update the address entry ourselves.
    pub quiet: Cell<bool>,
}

impl MainWindow {
    pub fn new(app: &adw::Application) -> Rc<MainWindow> {
        let m = state().session.borrow().messages();
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(m.new_tab)
            .default_width(1200)
            .default_height(800)
            .icon_name("llmouser")
            .build();

        let back = gtk::Button::from_icon_name("go-previous-symbolic");
        let forward = gtk::Button::from_icon_name("go-next-symbolic");
        let go = gtk::Button::from_icon_name("go-jump-symbolic");
        let settings_button = gtk::Button::from_icon_name("emblem-system-symbolic");
        let address = gtk::Entry::builder()
            .hexpand(true)
            .width_request(300)
            .build();
        address.set_input_purpose(gtk::InputPurpose::Url);
        let address_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        address_box.append(&address);
        address_box.append(&go);
        address_box.set_hexpand(true);

        let header = adw::HeaderBar::new();
        header.pack_start(&back);
        header.pack_start(&forward);
        header.set_title_widget(Some(&address_box));
        header.pack_end(&settings_button);

        let menubar = gtk::PopoverMenuBar::from_model(None::<&gio::MenuModel>);
        let tab_view = adw::TabView::new();
        let tab_bar = adw::TabBar::new();
        tab_bar.set_view(Some(&tab_view));
        tab_bar.set_autohide(false);
        let new_tab_button = gtk::Button::from_icon_name("tab-new-symbolic");
        new_tab_button.add_css_class("flat");
        tab_bar.set_end_action_widget(Some(&new_tab_button));

        let status = gtk::Label::new(Some(m.ready));
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        status.set_margin_start(10);
        status.set_margin_end(10);
        status.set_margin_top(3);
        status.set_margin_bottom(3);
        status.add_css_class("dim-label");
        status.add_css_class("caption");

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&menubar);
        toolbar_view.add_top_bar(&header);
        toolbar_view.add_top_bar(&tab_bar);
        toolbar_view.set_content(Some(&tab_view));
        toolbar_view.add_bottom_bar(&status);
        window.set_content(Some(&toolbar_view));

        let mw = Rc::new(MainWindow {
            window,
            tab_view,
            _tab_bar: tab_bar,
            menubar,
            address,
            back,
            forward,
            go,
            settings_button,
            new_tab_button,
            status,
            tabs: RefCell::new(Vec::new()),
            quiet: Cell::new(false),
        });
        mw.wire();
        mw
    }

    fn wire(self: &Rc<Self>) {
        self.back.connect_clicked(|_| menu::activate("back"));
        self.forward.connect_clicked(|_| menu::activate("forward"));
        self.settings_button
            .connect_clicked(|_| app::open_settings());
        self.new_tab_button
            .connect_clicked(|_| menu::activate("new-tab"));
        self.go.connect_clicked(|_| {
            let w = app::main_window();
            if w.current_entry()
                .map(|e| e.page.is_loading())
                .unwrap_or(false)
            {
                menu::activate("stop");
            } else {
                submit_address();
            }
        });
        self.address.connect_activate(|_| submit_address());
        self.address.connect_changed(|entry| {
            let w = app::main_window();
            if w.quiet.get() {
                return;
            }
            if let Some(tab) = w.current_tab() {
                state()
                    .session
                    .borrow_mut()
                    .browser
                    .set_address(tab, entry.text().as_str());
            }
        });
        self.tab_view.connect_selected_page_notify(|_| {
            let w = app::main_window();
            if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().browser.activate(tab);
                render(tab);
            }
        });
        self.tab_view.connect_close_page(|view, page| {
            let w = app::main_window();
            let tab = w
                .tabs
                .borrow()
                .iter()
                .find(|e| e.page == *page)
                .map(|e| e.tab);
            if let Some(tab) = tab {
                state().session.borrow_mut().close_tab(tab);
                w.tabs.borrow_mut().retain(|e| e.tab != tab);
            }
            view.close_page_finish(page, true);
            if view.n_pages() == 0 {
                // Like a browser: the window keeps one fresh tab.
                glib::idle_add_local_once(|| {
                    menu::activate("new-tab");
                });
            } else if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().browser.activate(tab);
                render(tab);
                menu::sync_enabled();
            }
            glib::Propagation::Stop
        });
    }

    pub fn create_tab(self: &Rc<Self>, tab: TabId) -> Rc<TabEntry> {
        let st = state();
        let webview = webview::create(&st.web, tab);
        let page = self.tab_view.append(&webview);
        page.set_title(st.session.borrow().messages().new_tab);
        let entry = Rc::new(TabEntry {
            tab,
            page: page.clone(),
            webview,
            loaded_seq: Cell::new(u64::MAX),
            expecting: RefCell::new(None),
            zoom: Cell::new(1.0),
            icon_kind: Cell::new("blank"),
        });
        self.tabs.borrow_mut().push(entry.clone());
        self.tab_view.set_selected_page(&page);
        self.address.grab_focus();
        entry
    }

    pub fn entry_for(&self, tab: TabId) -> Option<Rc<TabEntry>> {
        self.tabs.borrow().iter().find(|e| e.tab == tab).cloned()
    }

    pub fn current_entry(&self) -> Option<Rc<TabEntry>> {
        let page = self.tab_view.selected_page()?;
        self.tabs.borrow().iter().find(|e| e.page == page).cloned()
    }

    pub fn current_tab(&self) -> Option<TabId> {
        self.current_entry().map(|e| e.tab)
    }

    /// Tab entries in tab-bar order.
    pub fn ordered_entries(&self) -> Vec<Rc<TabEntry>> {
        let n = self.tab_view.n_pages();
        (0..n)
            .filter_map(|i| {
                let page = self.tab_view.nth_page(i);
                self.tabs.borrow().iter().find(|e| e.page == page).cloned()
            })
            .collect()
    }

    pub fn set_address_text(&self, text: &str) {
        if self.address.text().as_str() != text {
            self.quiet.set(true);
            self.address.set_text(text);
            self.quiet.set(false);
        }
    }

    pub fn focus_address(&self) {
        self.address.grab_focus();
    }
}

fn submit_address() {
    let w = app::main_window();
    let Some(tab) = w.current_tab() else { return };
    let text = w.address.text().to_string();
    state().session.borrow_mut().go(tab, &text);
    render(tab);
    if let Some(entry) = w.entry_for(tab) {
        entry.webview.grab_focus();
    }
}

/// Update every widget of a tab from the browser model.
pub fn render(tab: TabId) {
    let st = state();
    let w = app::main_window();
    let Some(entry) = w.entry_for(tab) else {
        return;
    };
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
            t.icon_svg(),
            if t.icon.is_some() {
                "favicon"
            } else if t.url.is_empty() {
                "blank"
            } else {
                "letter"
            },
            m,
        )
    };
    let (
        title,
        address,
        can_back,
        can_forward,
        loading,
        status,
        is_error,
        seq,
        document,
        icon_svg,
        icon_kind,
        m,
    ) = snapshot;

    entry.page.set_title(&title);
    entry.page.set_loading(loading);
    if entry.icon_kind.get() != icon_kind || seq != entry.loaded_seq.get() {
        let icon = gio::BytesIcon::new(&glib::Bytes::from_owned(icon_svg.into_bytes()));
        entry.page.set_icon(Some(&icon));
        entry.icon_kind.set(icon_kind);
    }

    let is_current = w.current_tab() == Some(tab);
    if is_current {
        w.window.set_title(Some(&title));
        w.set_address_text(&address);
        w.address.set_placeholder_text(Some(m.address_placeholder));
        w.address.set_tooltip_text(Some(m.address_bar));
        w.back.set_sensitive(can_back);
        w.forward.set_sensitive(can_forward);
        w.back.set_tooltip_text(Some(m.back));
        w.forward.set_tooltip_text(Some(m.forward));
        w.settings_button.set_tooltip_text(Some(m.settings));
        w.new_tab_button.set_tooltip_text(Some(m.new_tab));
        w.go.set_icon_name(if loading {
            "process-stop-symbolic"
        } else {
            "go-jump-symbolic"
        });
        w.go.set_tooltip_text(Some(if loading { m.stop } else { m.go }));
        w.status.set_text(&status);
        if is_error {
            w.status.add_css_class("error");
            w.status.remove_css_class("dim-label");
        } else {
            w.status.remove_css_class("error");
            w.status.add_css_class("dim-label");
        }
        menu::sync_enabled();
    }

    if entry.loaded_seq.get() != seq {
        entry.loaded_seq.set(seq);
        if let Some((html, base)) = document {
            load_document(&entry, &html, base.as_deref());
        }
    }
}

/// Empty tabs show the start page, which depends on whether a key is set.
pub fn refresh_empty() {
    let st = state();
    let w = app::main_window();
    for entry in w.tabs.borrow().iter() {
        let doc = {
            let s = st.session.borrow();
            match s.browser.tab(entry.tab) {
                Some(t) if matches!(t.content, llmouser_browser::Content::Empty) => {
                    s.document_for(entry.tab, false)
                }
                _ => None,
            }
        };
        if let Some((html, base)) = doc {
            load_document(entry, &html, base.as_deref());
        }
    }
}

fn load_document(entry: &TabEntry, html: &str, base: Option<&str>) {
    *entry.expecting.borrow_mut() = Some(
        base.map(str::to_string)
            .unwrap_or_else(|| "about:blank".into()),
    );
    entry.webview.load_html(html, base);
}

pub fn render_all() {
    let w = app::main_window();
    let tabs: Vec<TabId> = w.tabs.borrow().iter().map(|e| e.tab).collect();
    for tab in tabs {
        if let Some(entry) = w.entry_for(tab) {
            // Chrome pages (start/error) are translated too: force a reload.
            entry.loaded_seq.set(u64::MAX);
        }
        render(tab);
    }
}
