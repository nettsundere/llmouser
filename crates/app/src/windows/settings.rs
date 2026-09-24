//! The Settings window: a native tab control with LLM and UX pages.

use std::cell::Cell;
use std::rc::Rc;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, COLOR_BTNFACE, DEFAULT_GUI_FONT, HBRUSH};
use windows::Win32::System::SystemServices::SS_LEFT;
use windows::Win32::UI::Controls::EM_SETCUEBANNER;
use windows::Win32::UI::Controls::{
    NMHDR, TCIF_TEXT, TCITEMW, TCM_GETCURSEL, TCM_INSERTITEMW, TCM_SETCURSEL, TCM_SETITEMW,
    TCN_SELCHANGE, WC_TABCONTROLW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use llmouser_browser::automation::SettingsInfo;
use llmouser_browser::settings::Provider;
use llmouser_browser::Language;

use super::app::{self, state};
use super::util::{get_text, hiword, hs, loword, set_text};
use super::window::hinstance;
use crate::session::SettingsForm;

const ID_TABS: u16 = 201;
const ID_PROVIDER: u16 = 202;
const ID_ENDPOINT: u16 = 203;
const ID_MODEL: u16 = 204;
const ID_API_KEY: u16 = 205;
const ID_MAX_TOKENS: u16 = 206;
const ID_UNIVERSE: u16 = 207;
const ID_LANGUAGE: u16 = 208;
const ID_SEARCH_URL: u16 = 209;
const ID_SAVE: u16 = 210;
const ID_CLOSE: u16 = 211;

pub struct SettingsWindow {
    pub hwnd: HWND,
    tabs: HWND,
    labels: Vec<HWND>,
    pub provider: HWND,
    pub endpoint: HWND,
    pub model: HWND,
    pub api_key: HWND,
    pub max_tokens: HWND,
    pub universe: HWND,
    pub language: HWND,
    pub search_url: HWND,
    pub save: HWND,
    pub close: HWND,
    pub validation: HWND,
    open: Cell<bool>,
    has_api_key: Cell<bool>,
    quiet: Cell<bool>,
    api_key_placeholder: std::cell::RefCell<String>,
}

#[allow(clippy::too_many_arguments)]
unsafe fn control(
    class: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    parent: HWND,
    id: u16,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class,
        PCWSTR(hs(text).as_ptr()),
        WS_CHILD | style,
        x,
        y,
        w,
        h,
        Some(parent),
        Some(HMENU(id as usize as *mut _)),
        Some(hinstance()),
        None,
    )
    .expect("control");
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(
        hwnd,
        WM_SETFONT,
        Some(WPARAM(font.0 as usize)),
        Some(LPARAM(1)),
    );
    hwnd
}

impl SettingsWindow {
    pub fn new() -> Rc<SettingsWindow> {
        let m = state().session.borrow().messages();
        unsafe {
            let class_name = w!("LLMouserSettings");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as usize as *mut _),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME,
                class_name,
                PCWSTR(hs(m.settings_title).as_ptr()),
                WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                580,
                520,
                Some(app::main_window().hwnd),
                None,
                Some(hinstance()),
                None,
            )
            .expect("settings window");

            let tabs = control(
                WC_TABCONTROLW,
                "",
                WS_VISIBLE,
                hwnd,
                ID_TABS,
                10,
                10,
                545,
                400,
            );
            for (i, label) in [m.settings_tab_llm, m.settings_tab_ux].iter().enumerate() {
                let mut wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: PWSTR(wide.as_mut_ptr()),
                    ..Default::default()
                };
                SendMessageW(
                    tabs,
                    TCM_INSERTITEMW,
                    Some(WPARAM(i)),
                    Some(LPARAM(&item as *const _ as isize)),
                );
            }
            let label_style = WINDOW_STYLE(SS_LEFT.0);
            let edit_style = WS_BORDER | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32);
            let mut labels = Vec::new();
            let mut y = 50;
            let row = |y: &mut i32| {
                let current = *y;
                *y += 34;
                current
            };
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.provider,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let provider = control(
                w!("COMBOBOX"),
                "",
                WS_TABSTOP | WINDOW_STYLE((CBS_DROPDOWNLIST | WS_VSCROLL.0 as i32) as u32),
                hwnd,
                ID_PROVIDER,
                180,
                ry,
                360,
                200,
            );
            for p in Provider::ALL {
                SendMessageW(
                    provider,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(hs(p.label()).as_ptr() as isize)),
                );
            }
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.endpoint,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let endpoint = control(
                w!("EDIT"),
                "",
                edit_style,
                hwnd,
                ID_ENDPOINT,
                180,
                ry,
                360,
                24,
            );
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.model,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let model = control(w!("EDIT"), "", edit_style, hwnd, ID_MODEL, 180, ry, 360, 24);
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.api_key,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let api_key = control(
                w!("EDIT"),
                "",
                edit_style | WINDOW_STYLE(ES_PASSWORD as u32),
                hwnd,
                ID_API_KEY,
                180,
                ry,
                360,
                24,
            );
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.max_tokens,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let max_tokens = control(
                w!("EDIT"),
                "",
                edit_style | WINDOW_STYLE(ES_NUMBER as u32),
                hwnd,
                ID_MAX_TOKENS,
                180,
                ry,
                360,
                24,
            );
            let ry = row(&mut y);
            labels.push(control(
                w!("STATIC"),
                m.universe_rules,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let universe = control(
                w!("EDIT"),
                "",
                WS_BORDER
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN) as u32),
                hwnd,
                ID_UNIVERSE,
                180,
                ry,
                360,
                90,
            );

            // UX page (same coordinates, shown when that tab is selected).
            let mut uy = 50;
            let ry = row(&mut uy);
            labels.push(control(
                w!("STATIC"),
                m.language,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let language = control(
                w!("COMBOBOX"),
                "",
                WS_TABSTOP | WINDOW_STYLE((CBS_DROPDOWNLIST | WS_VSCROLL.0 as i32) as u32),
                hwnd,
                ID_LANGUAGE,
                180,
                ry,
                360,
                200,
            );
            for l in Language::ALL {
                SendMessageW(
                    language,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(hs(l.messages().language_name).as_ptr() as isize)),
                );
            }
            let ry = row(&mut uy);
            labels.push(control(
                w!("STATIC"),
                m.search_url,
                label_style,
                hwnd,
                0,
                24,
                ry + 4,
                150,
                20,
            ));
            let search_url = control(
                w!("EDIT"),
                "",
                edit_style,
                hwnd,
                ID_SEARCH_URL,
                180,
                ry,
                360,
                24,
            );
            let hint = control(
                w!("STATIC"),
                m.search_url_hint,
                label_style,
                hwnd,
                0,
                180,
                ry + 30,
                360,
                40,
            );
            labels.push(hint);

            let validation = control(w!("STATIC"), "", label_style, hwnd, 0, 10, 420, 545, 20);
            let save = control(
                w!("BUTTON"),
                m.save,
                WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                hwnd,
                ID_SAVE,
                360,
                448,
                90,
                28,
            );
            let close = control(
                w!("BUTTON"),
                m.close,
                WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                hwnd,
                ID_CLOSE,
                460,
                448,
                90,
                28,
            );
            let _ = ShowWindow(validation, SW_SHOW);

            let this = Rc::new(SettingsWindow {
                hwnd,
                tabs,
                labels,
                provider,
                endpoint,
                model,
                api_key,
                max_tokens,
                universe,
                language,
                search_url,
                save,
                close,
                validation,
                open: Cell::new(false),
                has_api_key: Cell::new(false),
                quiet: Cell::new(false),
                api_key_placeholder: std::cell::RefCell::new(String::new()),
            });
            this.show_page(0);
            this
        }
    }

    fn llm_controls(&self) -> Vec<HWND> {
        let mut v = vec![
            self.provider,
            self.endpoint,
            self.model,
            self.api_key,
            self.max_tokens,
            self.universe,
        ];
        v.extend(self.labels.iter().take(6).copied());
        v
    }

    fn ux_controls(&self) -> Vec<HWND> {
        let mut v = vec![self.language, self.search_url];
        v.extend(self.labels.iter().skip(6).copied());
        v
    }

    fn show_page(&self, page: usize) {
        unsafe {
            for c in self.llm_controls() {
                let _ = ShowWindow(c, if page == 0 { SW_SHOW } else { SW_HIDE });
            }
            for c in self.ux_controls() {
                let _ = ShowWindow(c, if page == 1 { SW_SHOW } else { SW_HIDE });
            }
        }
    }

    pub fn selected_page(&self) -> usize {
        let sel =
            unsafe { SendMessageW(self.tabs, TCM_GETCURSEL, Some(WPARAM(0)), Some(LPARAM(0))) }.0;
        sel.max(0) as usize
    }

    pub fn select_tab(&self, tab: &str) {
        let index = if tab == "ux" { 1 } else { 0 };
        unsafe {
            SendMessageW(
                self.tabs,
                TCM_SETCURSEL,
                Some(WPARAM(index)),
                Some(LPARAM(0)),
            );
        }
        self.show_page(index);
    }

    pub fn show(&self) {
        let (form, has_key) = {
            let st = state();
            let s = st.session.borrow();
            (s.settings_form(), s.public_settings().has_api_key)
        };
        self.has_api_key.set(has_key);
        self.apply(&form);
        self.select_tab("llm");
        set_text(self.validation, "");
        self.retranslate();
        self.open.set(true);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
        }
    }

    pub fn close(&self) {
        self.open.set(false);
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn apply(&self, form: &SettingsForm) {
        self.quiet.set(true);
        unsafe {
            SendMessageW(
                self.provider,
                CB_SETCURSEL,
                Some(WPARAM(
                    Provider::ALL
                        .iter()
                        .position(|p| *p == form.provider)
                        .unwrap_or(0),
                )),
                Some(LPARAM(0)),
            );
            SendMessageW(
                self.language,
                CB_SETCURSEL,
                Some(WPARAM(
                    Language::ALL
                        .iter()
                        .position(|l| *l == form.language)
                        .unwrap_or(0),
                )),
                Some(LPARAM(0)),
            );
        }
        set_text(self.endpoint, &form.endpoint);
        set_text(self.model, &form.model);
        set_text(self.api_key, "");
        set_text(self.max_tokens, &form.max_tokens);
        set_text(self.universe, &form.universe);
        set_text(self.search_url, &form.search_url);
        self.quiet.set(false);
    }

    pub fn form(&self) -> SettingsForm {
        SettingsForm {
            provider: self.picked_provider(),
            endpoint: get_text(self.endpoint),
            model: get_text(self.model),
            api_key: get_text(self.api_key),
            max_tokens: get_text(self.max_tokens),
            universe: get_text(self.universe).replace("\r\n", "\n"),
            language: self.picked_language(),
            search_url: get_text(self.search_url),
        }
    }

    pub fn picked_provider(&self) -> Provider {
        let i = unsafe {
            SendMessageW(
                self.provider,
                CB_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        }
        .0;
        Provider::ALL
            .get(i.max(0) as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn picked_language(&self) -> Language {
        let i = unsafe {
            SendMessageW(
                self.language,
                CB_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        }
        .0;
        Language::ALL
            .get(i.max(0) as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn provider_changed(&self) {
        let p = self.picked_provider();
        set_text(self.endpoint, p.default_endpoint());
        set_text(self.model, p.default_model());
    }

    pub fn show_validation(&self, text: &str) {
        set_text(self.validation, text);
    }

    fn save_clicked(&self) {
        let form = self.form();
        let before = state().session.borrow().language();
        let result = state().session.borrow_mut().save_settings(&form);
        match result {
            Ok(_) => {
                if form.language != before {
                    app::retranslate();
                }
                super::window::refresh_empty();
                self.close();
            }
            Err(problem) => self.show_validation(&problem),
        }
    }

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        set_text(self.hwnd, m.settings_title);
        unsafe {
            for (i, label) in [m.settings_tab_llm, m.settings_tab_ux].iter().enumerate() {
                let mut wide: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: PWSTR(wide.as_mut_ptr()),
                    ..Default::default()
                };
                SendMessageW(
                    self.tabs,
                    TCM_SETITEMW,
                    Some(WPARAM(i)),
                    Some(LPARAM(&item as *const _ as isize)),
                );
            }
        }
        let texts = [
            m.provider,
            m.endpoint,
            m.model,
            m.api_key,
            m.max_tokens,
            m.universe_rules,
            m.language,
            m.search_url,
            m.search_url_hint,
        ];
        for (label, text) in self.labels.iter().zip(texts) {
            set_text(*label, text);
        }
        *self.api_key_placeholder.borrow_mut() = (if self.has_api_key.get() {
            m.api_key_saved
        } else {
            m.api_key_enter
        })
        .to_string();
        unsafe {
            let placeholder: Vec<u16> = self
                .api_key_placeholder
                .borrow()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            SendMessageW(
                self.api_key,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(placeholder.as_ptr() as isize)),
            );
            let hint: Vec<u16> = m
                .universe_placeholder
                .encode_utf16()
                .chain(Some(0))
                .collect();
            SendMessageW(
                self.universe,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(hint.as_ptr() as isize)),
            );
        }
        set_text(self.save, m.save);
        set_text(self.close, m.close);
    }

    pub fn info(&self) -> SettingsInfo {
        let m = state().session.borrow().messages();
        SettingsInfo {
            open: self.is_open(),
            title: get_text(self.hwnd),
            tab: if self.selected_page() == 1 {
                "ux".into()
            } else {
                "llm".into()
            },
            tab_labels: vec![m.settings_tab_llm.into(), m.settings_tab_ux.into()],
            labels: self.labels.iter().take(8).map(|l| get_text(*l)).collect(),
            provider: self.picked_provider().id().into(),
            provider_options: Provider::ALL
                .iter()
                .map(|p| p.label().to_string())
                .collect(),
            endpoint: get_text(self.endpoint),
            model: get_text(self.model),
            api_key: get_text(self.api_key),
            api_key_placeholder: self.api_key_placeholder.borrow().clone(),
            max_tokens: get_text(self.max_tokens),
            universe: get_text(self.universe).replace("\r\n", "\n"),
            universe_placeholder: m.universe_placeholder.into(),
            language: self.picked_language().id().into(),
            language_options: Language::ALL
                .iter()
                .map(|l| l.messages().language_name.to_string())
                .collect(),
            search_url: get_text(self.search_url),
            save_label: get_text(self.save),
            close_label: get_text(self.close),
            validation: get_text(self.validation),
        }
    }

    pub fn set_field(&self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "provider" => {
                let p =
                    Provider::from_id(value).ok_or_else(|| format!("unknown provider {value}"))?;
                unsafe {
                    SendMessageW(
                        self.provider,
                        CB_SETCURSEL,
                        Some(WPARAM(
                            Provider::ALL.iter().position(|x| *x == p).unwrap_or(0),
                        )),
                        Some(LPARAM(0)),
                    );
                }
                self.provider_changed();
            }
            "language" => {
                let l =
                    Language::from_id(value).ok_or_else(|| format!("unknown language {value}"))?;
                unsafe {
                    SendMessageW(
                        self.language,
                        CB_SETCURSEL,
                        Some(WPARAM(
                            Language::ALL.iter().position(|x| *x == l).unwrap_or(0),
                        )),
                        Some(LPARAM(0)),
                    );
                }
                app::set_language(l);
            }
            "endpoint" => set_text(self.endpoint, value),
            "model" => set_text(self.model, value),
            "api_key" => set_text(self.api_key, value),
            "max_tokens" => set_text(self.max_tokens, value),
            "universe" => set_text(self.universe, &value.replace('\n', "\r\n")),
            "search_url" => set_text(self.search_url, value),
            other => return Err(format!("unknown settings field {other}")),
        }
        Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let settings = state().settings.borrow().clone();
    match msg {
        WM_COMMAND => {
            let id = loword(wparam.0);
            let code = hiword(wparam.0);
            if let Some(s) = settings {
                match id {
                    ID_SAVE => s.save_clicked(),
                    ID_CLOSE => s.close(),
                    ID_PROVIDER if code == CBN_SELCHANGE as u16 && !s.quiet.get() => {
                        s.provider_changed()
                    }
                    ID_LANGUAGE if code == CBN_SELCHANGE as u16 && !s.quiet.get() => {
                        app::set_language(s.picked_language())
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let hdr = &*(lparam.0 as *const NMHDR);
            if hdr.idFrom == ID_TABS as usize && hdr.code == TCN_SELCHANGE {
                if let Some(s) = settings {
                    s.show_page(s.selected_page());
                }
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(s) = settings {
                s.close();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
