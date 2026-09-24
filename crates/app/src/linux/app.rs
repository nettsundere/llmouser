//! Application state and lifecycle.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};
use libadwaita as adw;
use webkit6 as webkit;

use llmouser_browser::{Language, TabId};

use super::about::AboutDialog;
use super::settings::SettingsDialog;
use super::window::MainWindow;
use super::{automation, menu, webview};
use crate::session::{self, Session};

pub const APP_ID: &str = "com.nettsundere.llmouser";
/// Full-bleed icon variant used on Linux.
pub const ICON_PNG: &[u8] = include_bytes!("../../../../assets/icon.png");

pub struct State {
    pub app: adw::Application,
    pub session: RefCell<Session>,
    pub window: RefCell<Option<Rc<MainWindow>>>,
    pub settings: RefCell<Option<Rc<SettingsDialog>>>,
    pub about: RefCell<Option<Rc<AboutDialog>>>,
    pub web: webview::WebContext,
    pub pending_print: RefCell<Option<webkit::PrintOperation>>,
}

thread_local! {
    static STATE: RefCell<Option<Rc<State>>> = const { RefCell::new(None) };
}

pub fn state() -> Rc<State> {
    STATE.with(|s| s.borrow().clone().expect("app state initialised"))
}

/// Run a closure on the GTK main loop, from any thread.
pub fn on_main(f: impl FnOnce() + Send + 'static) {
    glib::MainContext::default().invoke(f);
}

pub fn run() {
    let testing = std::env::var(llmouser_browser::env::E2E_PORT).is_ok();
    let flags = if testing {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::default()
    };
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    app.connect_activate(activate);
    // GTK must not parse our command line.
    let args: [&str; 0] = [];
    app.run_with_args(&args);
}

fn activate(app: &adw::Application) {
    if state_exists() {
        // Second activation (e.g. reopen): just present the window.
        if let Some(w) = state().window.borrow().as_ref() {
            w.window.present();
        }
        return;
    }
    let session = Session::new(session::sink(|event| {
        on_main(move || {
            let st = state();
            let changed = st.session.borrow_mut().on_event(event);
            if let Some(tab) = changed {
                super::window::render(tab);
            }
        })
    }));
    let web = webview::WebContext::new(session.store.path().parent().map(|p| p.to_path_buf()));
    let st = Rc::new(State {
        app: app.clone(),
        session: RefCell::new(session),
        window: RefCell::new(None),
        settings: RefCell::new(None),
        about: RefCell::new(None),
        web,
        pending_print: RefCell::new(None),
    });
    STATE.with(|s| *s.borrow_mut() = Some(st.clone()));

    menu::install_actions(app.upcast_ref::<gtk::Application>());
    let window = MainWindow::new(app);
    *st.window.borrow_mut() = Some(window.clone());
    menu::install(st.session.borrow().language());
    let tab = st.session.borrow().browser.active_id();
    window.create_tab(tab);
    super::window::render(tab);
    window.window.present();

    if let Some(port) = std::env::var(llmouser_browser::env::E2E_PORT)
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        if let Err(e) = automation::start(port) {
            eprintln!("automation server failed on port {port}: {e}");
        }
    }
}

fn state_exists() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

pub fn main_window() -> Rc<MainWindow> {
    state().window.borrow().clone().expect("main window")
}

/// The tab shown in the window right now.
pub fn current_tab() -> Option<TabId> {
    main_window().current_tab()
}

pub fn open_settings() {
    let st = state();
    let existing = st.settings.borrow().clone();
    let dialog = match existing {
        Some(d) => d,
        None => {
            let d = SettingsDialog::new();
            *st.settings.borrow_mut() = Some(d.clone());
            d
        }
    };
    dialog.show();
}

pub fn open_about() {
    let st = state();
    let existing = st.about.borrow().clone();
    let dialog = match existing {
        Some(d) => d,
        None => {
            let d = AboutDialog::new();
            *st.about.borrow_mut() = Some(d.clone());
            d
        }
    };
    dialog.show();
}

pub fn set_language(language: Language) {
    let st = state();
    if st.session.borrow().language() == language {
        return;
    }
    if let Err(e) = st.session.borrow_mut().set_language(language) {
        eprintln!("could not save language: {e}");
    }
    retranslate();
}

pub fn retranslate() {
    let st = state();
    menu::install(st.session.borrow().language());
    super::window::render_all();
    let settings = st.settings.borrow().clone();
    if let Some(s) = settings {
        s.retranslate();
    }
    let about = st.about.borrow().clone();
    if let Some(a) = about {
        a.retranslate();
    }
}
