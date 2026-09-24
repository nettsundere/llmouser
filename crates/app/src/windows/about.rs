//! The About window: centered icon (from the executable's resources), name,
//! version and copyright.

use std::cell::Cell;
use std::rc::Rc;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, COLOR_WINDOW, DEFAULT_GUI_FONT, HBRUSH};
use windows::Win32::System::SystemServices::{
    SS_CENTER, SS_CENTERIMAGE, SS_ICON, SS_REALSIZECONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use llmouser_browser::automation::AboutInfo;
use llmouser_browser::i18n::fill;
use llmouser_browser::{APP_COPYRIGHT, APP_NAME, APP_VERSION};

use super::app::{self, state};
use super::util::{get_text, hs, set_text};
use super::window::hinstance;

const WIDTH: i32 = 300;
const HEIGHT: i32 = 330;

pub struct AboutWindow {
    pub hwnd: HWND,
    icon: HWND,
    name: HWND,
    version: HWND,
    copyright: HWND,
    open: Cell<bool>,
    icon_loaded: bool,
}

impl AboutWindow {
    pub fn new() -> Rc<AboutWindow> {
        unsafe {
            let class_name = w!("LLMouserAbout");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinstance(),
                lpszClassName: class_name,
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut _),
                ..Default::default()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                WS_EX_DLGMODALFRAME,
                class_name,
                PCWSTR::null(),
                WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WIDTH,
                HEIGHT,
                Some(app::main_window().hwnd),
                None,
                Some(hinstance()),
                None,
            )
            .expect("about window");
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let cw = rc.right - rc.left;

            let font = GetStockObject(DEFAULT_GUI_FONT);
            let make =
                |class: PCWSTR, text: &str, style: u32, x: i32, y: i32, w: i32, h: i32| -> HWND {
                    let c = CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        class,
                        PCWSTR(hs(text).as_ptr()),
                        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(style),
                        x,
                        y,
                        w,
                        h,
                        Some(hwnd),
                        None,
                        Some(hinstance()),
                        None,
                    )
                    .expect("control");
                    SendMessageW(
                        c,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                    c
                };
            let icon = make(
                w!("STATIC"),
                "",
                SS_ICON.0 | SS_CENTERIMAGE.0 | SS_REALSIZECONTROL.0,
                (cw - 128) / 2,
                30,
                128,
                128,
            );
            let image = LoadImageW(
                Some(hinstance()),
                PCWSTR(1 as *const u16),
                IMAGE_ICON,
                128,
                128,
                LR_DEFAULTCOLOR,
            );
            let icon_loaded = match image {
                Ok(handle) if !handle.is_invalid() => {
                    SendMessageW(
                        icon,
                        STM_SETIMAGE,
                        Some(WPARAM(IMAGE_ICON.0 as usize)),
                        Some(LPARAM(handle.0 as isize)),
                    );
                    true
                }
                _ => false,
            };
            let name = make(w!("STATIC"), APP_NAME, SS_CENTER.0, 0, 172, cw, 22);
            let version = make(w!("STATIC"), "", SS_CENTER.0, 0, 198, cw, 20);
            let copyright = make(w!("STATIC"), APP_COPYRIGHT, SS_CENTER.0, 0, 220, cw, 20);
            let this = Rc::new(AboutWindow {
                hwnd,
                icon,
                name,
                version,
                copyright,
                open: Cell::new(false),
                icon_loaded,
            });
            this.retranslate();
            this
        }
    }

    pub fn show(&self) {
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

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        set_text(self.hwnd, &fill(m.menu_about, &[("app", APP_NAME)]));
        set_text(
            self.version,
            &format!("{} {}", m.about_version, APP_VERSION),
        );
    }

    pub fn info(&self) -> AboutInfo {
        let off_center = unsafe {
            let mut icon_rc = RECT::default();
            let _ = GetWindowRect(self.icon, &mut icon_rc);
            let mut win_rc = RECT::default();
            let _ = GetClientRect(self.hwnd, &mut win_rc);
            let mut origin = windows::Win32::Foundation::POINT::default();
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(self.hwnd, &mut origin);
            let icon_center = (icon_rc.left + icon_rc.right) as f64 / 2.0 - origin.x as f64;
            (icon_center - (win_rc.right - win_rc.left) as f64 / 2.0).abs()
        };
        AboutInfo {
            open: self.is_open(),
            title: get_text(self.hwnd),
            name: get_text(self.name),
            version_line: get_text(self.version),
            copyright: get_text(self.copyright),
            icon_loaded: self.icon_loaded,
            icon_off_center: off_center,
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => {
            if let Some(a) = state().about.borrow().clone() {
                a.close();
            }
            LRESULT(0)
        }
        WM_KEYDOWN
            if wparam.0 as u16 == windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0 =>
        {
            if let Some(a) = state().about.borrow().clone() {
                a.close();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
