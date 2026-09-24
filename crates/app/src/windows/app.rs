//! Application state, the message loop and main-thread dispatch.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES,
    INITCOMMONCONTROLSEX,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PostMessageW, TranslateAcceleratorW, TranslateMessage, MSG,
    WM_APP,
};

use llmouser_browser::{Language, TabId};

use super::about::AboutWindow;
use super::settings::SettingsWindow;
use super::window::MainWindow;
use super::{automation, menu, webview};
use crate::session::{self, Session};

/// Message posted to the main window to drain the dispatch queue.
pub const WM_DISPATCH: u32 = WM_APP + 1;

pub struct State {
    pub session: RefCell<Session>,
    pub window: RefCell<Option<Rc<MainWindow>>>,
    pub settings: RefCell<Option<Rc<SettingsWindow>>>,
    pub about: RefCell<Option<Rc<AboutWindow>>>,
    pub env: RefCell<Option<ICoreWebView2Environment>>,
    pub context_menu_open: RefCell<Vec<(String, String)>>,
}

thread_local! {
    static STATE: RefCell<Option<Rc<State>>> = const { RefCell::new(None) };
}

static QUEUE: Mutex<VecDeque<Box<dyn FnOnce() + Send>>> = Mutex::new(VecDeque::new());
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);

pub fn state() -> Rc<State> {
    STATE.with(|s| s.borrow().clone().expect("app state initialised"))
}

/// Run a closure on the UI thread, from any thread.
pub fn on_main(f: impl FnOnce() + Send + 'static) {
    QUEUE.lock().expect("dispatch queue").push_back(Box::new(f));
    let hwnd = MAIN_HWND.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd as *mut _)),
                WM_DISPATCH,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

/// Called by the main window on `WM_DISPATCH`.
pub fn drain_queue() {
    loop {
        let next = QUEUE.lock().expect("dispatch queue").pop_front();
        match next {
            Some(f) => f(),
            None => break,
        }
    }
}

pub fn run() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_TAB_CLASSES | ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
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
    let st = Rc::new(State {
        session: RefCell::new(session),
        window: RefCell::new(None),
        settings: RefCell::new(None),
        about: RefCell::new(None),
        env: RefCell::new(None),
        context_menu_open: RefCell::new(Vec::new()),
    });
    STATE.with(|s| *s.borrow_mut() = Some(st.clone()));

    let window = MainWindow::new();
    MAIN_HWND.store(window.hwnd.0 as isize, Ordering::SeqCst);
    *st.window.borrow_mut() = Some(window.clone());
    menu::install(st.session.borrow().language());
    let tab = st.session.borrow().browser.active_id();
    window.create_tab(tab);
    super::window::render(tab);
    webview::create_environment();

    if let Some(port) = std::env::var(llmouser_browser::env::E2E_PORT)
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
    {
        if let Err(e) = automation::start(port) {
            eprintln!("automation server failed on port {port}: {e}");
        }
    }
    drain_queue();

    let accel = menu::accelerators();
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if TranslateAcceleratorW(window.hwnd, accel, &msg) != 0 {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

pub fn main_window() -> Rc<MainWindow> {
    state().window.borrow().clone().expect("main window")
}

pub fn current_tab() -> Option<TabId> {
    main_window().current_tab()
}

pub fn open_settings() {
    let st = state();
    let existing = st.settings.borrow().clone();
    let window = match existing {
        Some(w) => w,
        None => {
            let w = SettingsWindow::new();
            *st.settings.borrow_mut() = Some(w.clone());
            w
        }
    };
    window.show();
}

pub fn open_about() {
    let st = state();
    let existing = st.about.borrow().clone();
    let window = match existing {
        Some(w) => w,
        None => {
            let w = AboutWindow::new();
            *st.about.borrow_mut() = Some(w.clone());
            w
        }
    };
    window.show();
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
