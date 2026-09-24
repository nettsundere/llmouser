//! The browser window: a toolbar row (back, forward, address, go/stop,
//! settings), a native tab control, the WebView2 area and a status line.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2Controller};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{SS_ENDELLIPSIS, SS_LEFT};
use windows::Win32::UI::Controls::{
    NMHDR, TCIF_TEXT, TCITEMW, TCM_DELETEITEM, TCM_GETCURSEL, TCM_INSERTITEMW, TCM_SETCURSEL,
    TCM_SETITEMW, TCN_SELCHANGE, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_UPDATETIPTEXTW,
    TTS_ALWAYSTIP, TTTOOLINFOW, WC_TABCONTROLW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_RETURN};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

use llmouser_browser::TabId;

use super::app::{self, state, WM_DISPATCH};
use super::util::{get_text, hiword, hs, loword, set_text};
use super::{menu, webview};

pub const ID_BACK: u16 = 101;
pub const ID_FORWARD: u16 = 102;
pub const ID_ADDRESS: u16 = 103;
pub const ID_GO: u16 = 104;
pub const ID_SETTINGS: u16 = 105;
pub const ID_NEW_TAB: u16 = 106;
pub const ID_CLOSE_TAB: u16 = 107;
pub const ID_TABS: u16 = 108;

const TOOLBAR_HEIGHT: i32 = 36;
const TABS_HEIGHT: i32 = 26;
const STATUS_HEIGHT: i32 = 22;

pub struct TabEntry {
    pub tab: TabId,
    /// Child window hosting this tab's WebView2 controller. WebView2 allows
    /// exactly one controller per HWND, so each tab needs its own host window.
    pub host: HWND,
    pub controller: RefCell<Option<ICoreWebView2Controller>>,
    pub webview: RefCell<Option<ICoreWebView2>>,
    pub loaded_seq: Cell<u64>,
    pub zoom: Cell<f64>,
    /// Document to load once the WebView2 controller exists.
    pub pending: RefCell<Option<String>>,
}

pub struct MainWindow {
    pub hwnd: HWND,
    pub back: HWND,
    pub forward: HWND,
    pub address: HWND,
    pub go: HWND,
    pub settings_button: HWND,
    pub new_tab_button: HWND,
    pub close_tab_button: HWND,
    pub tabs_ctl: HWND,
    pub status: HWND,
    pub tooltips: HWND,
    pub tabs: RefCell<Vec<Rc<TabEntry>>>,
    pub quiet: Cell<bool>,
    pub loading: Cell<bool>,
    pub status_error: Cell<bool>,
    pub tooltip_texts: RefCell<Vec<String>>,
    pub placeholder: RefCell<String>,
}

pub fn hinstance() -> HINSTANCE {
    unsafe {
        GetModuleHandleW(None)
            .map(|m| HINSTANCE(m.0))
            .unwrap_or_default()
    }
}

unsafe fn child(class: PCWSTR, text: &str, style: WINDOW_STYLE, parent: HWND, id: u16) -> HWND {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class,
        PCWSTR(hs(text).as_ptr()),
        WS_CHILD | WS_VISIBLE | style,
        0,
        0,
        10,
        10,
        Some(parent),
        Some(HMENU(id as usize as *mut _)),
        Some(hinstance()),
        None,
    )
    .expect("child control");
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(
        hwnd,
        WM_SETFONT,
        Some(WPARAM(font.0 as usize)),
        Some(LPARAM(1)),
    );
    hwnd
}

impl MainWindow {
    pub fn new() -> Rc<MainWindow> {
        let m = state().session.borrow().messages();
        unsafe {
            let class_name = w!("LLMouserMainWindow");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut _),
                // 1 = the embedded icon resource id (not a dereferenced pointer).
                #[allow(clippy::manual_dangling_ptr)]
                hIcon: LoadIconW(Some(hinstance()), PCWSTR(1 as *const u16)).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                PCWSTR(hs(m.new_tab).as_ptr()),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1200,
                800,
                None,
                None,
                Some(hinstance()),
                None,
            )
            .expect("main window");

            let back = child(
                w!("BUTTON"),
                "\u{2190}",
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_BACK,
            );
            let forward = child(
                w!("BUTTON"),
                "\u{2192}",
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_FORWARD,
            );
            let address = child(
                w!("EDIT"),
                "",
                WS_BORDER | WINDOW_STYLE((ES_AUTOHSCROLL | ES_LEFT) as u32),
                hwnd,
                ID_ADDRESS,
            );
            let _ = SetWindowSubclass(address, Some(address_proc), 1, 0);
            let go = child(
                w!("BUTTON"),
                m.go,
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_GO,
            );
            let settings_button = child(
                w!("BUTTON"),
                "\u{2699}",
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_SETTINGS,
            );
            let tabs_ctl = child(WC_TABCONTROLW, "", WS_CLIPSIBLINGS, hwnd, ID_TABS);
            let new_tab_button = child(
                w!("BUTTON"),
                "+",
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_NEW_TAB,
            );
            let close_tab_button = child(
                w!("BUTTON"),
                "\u{00d7}",
                WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_CLOSE_TAB,
            );
            let status = child(
                w!("STATIC"),
                m.ready,
                WINDOW_STYLE(SS_LEFT.0 | SS_ENDELLIPSIS.0),
                hwnd,
                0,
            );
            let tooltips = CreateWindowExW(
                WS_EX_TOPMOST,
                w!("tooltips_class32"),
                None,
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                Some(hwnd),
                None,
                Some(hinstance()),
                None,
            )
            .unwrap_or_default();

            let mw = Rc::new(MainWindow {
                hwnd,
                back,
                forward,
                address,
                go,
                settings_button,
                new_tab_button,
                close_tab_button,
                tabs_ctl,
                status,
                tooltips,
                tabs: RefCell::new(Vec::new()),
                quiet: Cell::new(false),
                loading: Cell::new(false),
                status_error: Cell::new(false),
                tooltip_texts: RefCell::new(vec![String::new(); 5]),
                placeholder: RefCell::new(String::new()),
            });
            for (i, control) in [back, forward, go, settings_button, new_tab_button]
                .iter()
                .enumerate()
            {
                let mut text: Vec<u16> = "\0".encode_utf16().collect();
                let info = TTTOOLINFOW {
                    cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                    uFlags: TTF_IDISHWND | TTF_SUBCLASS,
                    hwnd,
                    uId: control.0 as usize,
                    lpszText: PWSTR(text.as_mut_ptr()),
                    ..Default::default()
                };
                let _ = i;
                SendMessageW(
                    tooltips,
                    TTM_ADDTOOLW,
                    Some(WPARAM(0)),
                    Some(LPARAM(&info as *const _ as isize)),
                );
            }
            mw.layout();
            mw
        }
    }

    pub fn set_tooltip(&self, index: usize, control: HWND, text: &str) {
        self.tooltip_texts.borrow_mut()[index] = text.to_string();
        unsafe {
            let mut wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            let info = TTTOOLINFOW {
                cbSize: std::mem::size_of::<TTTOOLINFOW>() as u32,
                uFlags: TTF_IDISHWND | TTF_SUBCLASS,
                hwnd: self.hwnd,
                uId: control.0 as usize,
                lpszText: PWSTR(wide.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                self.tooltips,
                TTM_UPDATETIPTEXTW,
                Some(WPARAM(0)),
                Some(LPARAM(&info as *const _ as isize)),
            );
        }
    }

    pub fn layout(&self) {
        unsafe {
            let mut rc = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut rc);
            let width = rc.right - rc.left;
            let height = rc.bottom - rc.top;
            let y = 4;
            let h = TOOLBAR_HEIGHT - 8;
            let _ = MoveWindow(self.back, 4, y, 32, h, true);
            let _ = MoveWindow(self.forward, 40, y, 32, h, true);
            let _ = MoveWindow(
                self.address,
                80,
                y + 3,
                width - 80 - 48 - 44 - 8,
                h - 6,
                true,
            );
            let _ = MoveWindow(self.go, width - 48 - 44, y, 44, h, true);
            let _ = MoveWindow(self.settings_button, width - 40, y, 36, h, true);
            let tabs_y = TOOLBAR_HEIGHT;
            let _ = MoveWindow(self.tabs_ctl, 0, tabs_y, width - 64, TABS_HEIGHT, true);
            let _ = MoveWindow(
                self.new_tab_button,
                width - 62,
                tabs_y + 1,
                28,
                TABS_HEIGHT - 2,
                true,
            );
            let _ = MoveWindow(
                self.close_tab_button,
                width - 32,
                tabs_y + 1,
                28,
                TABS_HEIGHT - 2,
                true,
            );
            let content_top = TOOLBAR_HEIGHT + TABS_HEIGHT;
            let content_bottom = height - STATUS_HEIGHT;
            let _ = MoveWindow(
                self.status,
                8,
                content_bottom + 3,
                width - 16,
                STATUS_HEIGHT - 4,
                true,
            );
            let current = self.current_tab();
            let content_height = content_bottom - content_top;
            for entry in self.tabs.borrow().iter() {
                let visible = current == Some(entry.tab);
                let _ = MoveWindow(entry.host, 0, content_top, width, content_height, true);
                let _ = ShowWindow(entry.host, if visible { SW_SHOW } else { SW_HIDE });
                if let Some(controller) = entry.controller.borrow().as_ref() {
                    let _ = controller.SetBounds(RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: content_height,
                    });
                    let _ = controller.SetIsVisible(visible);
                }
            }
        }
    }

    pub fn create_tab(self: &Rc<Self>, tab: TabId) -> Rc<TabEntry> {
        let m = state().session.borrow().messages();
        let host = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                10,
                10,
                Some(self.hwnd),
                None,
                Some(hinstance()),
                None,
            )
            .expect("tab host window")
        };
        let entry = Rc::new(TabEntry {
            tab,
            host,
            controller: RefCell::new(None),
            webview: RefCell::new(None),
            loaded_seq: Cell::new(u64::MAX),
            zoom: Cell::new(1.0),
            pending: RefCell::new(None),
        });
        let index = self.tabs.borrow().len();
        self.tabs.borrow_mut().push(entry.clone());
        unsafe {
            let mut title: Vec<u16> = m.new_tab.encode_utf16().chain(Some(0)).collect();
            let item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(title.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                self.tabs_ctl,
                TCM_INSERTITEMW,
                Some(WPARAM(index)),
                Some(LPARAM(&item as *const _ as isize)),
            );
            SendMessageW(
                self.tabs_ctl,
                TCM_SETCURSEL,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
        }
        if let Some(env) = state().env.borrow().clone() {
            webview::create(&env, entry.host, entry.clone());
        }
        self.layout();
        self.focus_address();
        entry
    }

    pub fn ensure_webviews(self: &Rc<Self>) {
        let Some(env) = state().env.borrow().clone() else {
            return;
        };
        for entry in self.tabs.borrow().iter() {
            if entry.controller.borrow().is_none() {
                webview::create(&env, entry.host, entry.clone());
            }
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        let sel = unsafe {
            SendMessageW(
                self.tabs_ctl,
                TCM_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        }
        .0;
        (sel >= 0).then_some(sel as usize)
    }

    pub fn current_entry(&self) -> Option<Rc<TabEntry>> {
        let index = self.selected_index()?;
        self.tabs.borrow().get(index).cloned()
    }

    pub fn current_tab(&self) -> Option<TabId> {
        self.current_entry().map(|e| e.tab)
    }

    pub fn entry_for(&self, tab: TabId) -> Option<Rc<TabEntry>> {
        self.tabs.borrow().iter().find(|e| e.tab == tab).cloned()
    }

    pub fn select_index(&self, index: usize) {
        unsafe {
            SendMessageW(
                self.tabs_ctl,
                TCM_SETCURSEL,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
        }
        self.tab_selected();
    }

    /// The tab control's selection changed (by the user or by us).
    pub fn tab_selected(&self) {
        if let Some(tab) = self.current_tab() {
            state().session.borrow_mut().browser.activate(tab);
            self.layout();
            render(tab);
        }
    }

    pub fn close_index(self: &Rc<Self>, index: usize) {
        let entry = self.tabs.borrow().get(index).cloned();
        let Some(entry) = entry else { return };
        state().session.borrow_mut().close_tab(entry.tab);
        if let Some(controller) = entry.controller.borrow_mut().take() {
            unsafe {
                let _ = controller.Close();
            }
        }
        unsafe {
            let _ = DestroyWindow(entry.host);
        }
        self.tabs.borrow_mut().remove(index);
        unsafe {
            SendMessageW(
                self.tabs_ctl,
                TCM_DELETEITEM,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
        }
        if self.tabs.borrow().is_empty() {
            let tab = state().session.borrow_mut().browser.new_tab();
            self.create_tab(tab);
            render(tab);
        } else {
            let next = index.min(self.tabs.borrow().len() - 1);
            self.select_index(next);
        }
        menu::sync_enabled();
    }

    pub fn set_address_text(&self, text: &str) {
        if get_text(self.address) != text {
            self.quiet.set(true);
            set_text(self.address, text);
            self.quiet.set(false);
        }
    }

    pub fn focus_address(&self) {
        unsafe {
            let _ = SetFocus(Some(self.address));
        }
    }

    pub fn set_tab_title(&self, index: usize, title: &str) {
        unsafe {
            let mut wide: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
            let item = TCITEMW {
                mask: TCIF_TEXT,
                pszText: PWSTR(wide.as_mut_ptr()),
                ..Default::default()
            };
            SendMessageW(
                self.tabs_ctl,
                TCM_SETITEMW,
                Some(WPARAM(index)),
                Some(LPARAM(&item as *const _ as isize)),
            );
        }
    }
}

pub fn submit_address() {
    let w = app::main_window();
    let Some(tab) = w.current_tab() else { return };
    let text = get_text(w.address);
    state().session.borrow_mut().go(tab, &text);
    render(tab);
}

unsafe extern "system" fn address_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_KEYDOWN && wparam.0 as u16 == VK_RETURN.0 {
        submit_address();
        return LRESULT(0);
    }
    if msg == WM_CHAR && wparam.0 == 13 {
        return LRESULT(0);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // A panic must never cross this `extern "system"` boundary (it would abort).
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        handle_message(hwnd, msg, wparam, lparam)
    })) {
        Ok(result) => result,
        Err(_) => {
            eprintln!("window procedure panicked on message {msg:#x}");
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// The main window, if it has been registered yet (messages arrive during creation).
fn window_opt() -> Option<Rc<MainWindow>> {
    app::state_opt().and_then(|st| st.window.borrow().clone())
}

unsafe fn handle_message(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DISPATCH => {
            app::drain_queue();
            LRESULT(0)
        }
        WM_SIZE => {
            if let Some(w) = window_opt() {
                w.layout();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            if window_opt().is_some() {
                command(loword(wparam.0), hiword(wparam.0));
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let hdr = &*(lparam.0 as *const NMHDR);
            if hdr.idFrom == ID_TABS as usize && hdr.code == TCN_SELCHANGE {
                if let Some(w) = window_opt() {
                    w.tab_selected();
                }
            }
            LRESULT(0)
        }
        WM_SETFOCUS => {
            if let Some(entry) = window_opt().and_then(|w| w.current_entry()) {
                if let Some(controller) = entry.controller.borrow().as_ref() {
                    let _ = controller.MoveFocus(webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn command(id: u16, code: u16) {
    match id {
        ID_BACK => menu::perform("back"),
        ID_FORWARD => menu::perform("forward"),
        ID_GO => {
            if app::main_window().loading.get() {
                menu::perform("stop");
            } else {
                submit_address();
            }
        }
        ID_SETTINGS => app::open_settings(),
        ID_NEW_TAB => menu::perform("new-tab"),
        ID_CLOSE_TAB => menu::perform("close-tab"),
        ID_ADDRESS => {
            if code == EN_CHANGE as u16 {
                let w = app::main_window();
                if !w.quiet.get() {
                    if let Some(tab) = w.current_tab() {
                        state()
                            .session
                            .borrow_mut()
                            .browser
                            .set_address(tab, &get_text(w.address));
                    }
                }
            }
        }
        other => menu::command(other),
    }
}

/// Update every widget of a tab from the browser model.
pub fn render(tab: TabId) {
    let st = state();
    let w = app::main_window();
    let Some(entry) = w.entry_for(tab) else {
        return;
    };
    let index = w
        .tabs
        .borrow()
        .iter()
        .position(|e| e.tab == tab)
        .unwrap_or(0);
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
            s.document_for(tab, true),
            m,
        )
    };
    let (title, address, can_back, can_forward, loading, status, is_error, seq, document, m) =
        snapshot;

    w.set_tab_title(
        index,
        &if loading {
            format!("{title} …")
        } else {
            title.clone()
        },
    );
    if w.current_tab() == Some(tab) {
        set_text(w.hwnd, &title);
        w.set_address_text(&address);
        *w.placeholder.borrow_mut() = m.address_placeholder.to_string();
        unsafe {
            let _ = EnableWindow(w.back, can_back);
            let _ = EnableWindow(w.forward, can_forward);
        }
        w.loading.set(loading);
        set_text(w.go, if loading { m.stop } else { m.go });
        w.set_tooltip(0, w.back, m.back);
        w.set_tooltip(1, w.forward, m.forward);
        w.set_tooltip(2, w.go, if loading { m.stop } else { m.go });
        w.set_tooltip(3, w.settings_button, m.settings);
        w.set_tooltip(4, w.new_tab_button, m.new_tab);
        set_text(w.status, &status);
        w.status_error.set(is_error);
        menu::sync_enabled();
    }
    if entry.loaded_seq.get() != seq {
        entry.loaded_seq.set(seq);
        if let Some((html, _)) = document {
            load_document(&entry, html);
        }
    }
}

pub fn load_document(entry: &TabEntry, html: String) {
    let webview = entry.webview.borrow().clone();
    match webview {
        Some(wv) => unsafe {
            let _ = wv.NavigateToString(PCWSTR(hs(&html).as_ptr()));
        },
        None => *entry.pending.borrow_mut() = Some(html),
    }
}

pub fn refresh_empty() {
    let st = state();
    let w = app::main_window();
    for entry in w.tabs.borrow().iter() {
        let doc = {
            let s = st.session.borrow();
            match s.browser.tab(entry.tab) {
                Some(t) if matches!(t.content, llmouser_browser::Content::Empty) => {
                    s.document_for(entry.tab, true)
                }
                _ => None,
            }
        };
        if let Some((html, _)) = doc {
            load_document(entry, html);
        }
    }
}

pub fn render_all() {
    let w = app::main_window();
    let tabs: Vec<TabId> = w.tabs.borrow().iter().map(|e| e.tab).collect();
    for tab in tabs {
        if let Some(entry) = w.entry_for(tab) {
            entry.loaded_seq.set(u64::MAX);
        }
        render(tab);
    }
}
