//! The menu bar, accelerator table and context menu, rebuilt on language change.

use std::cell::RefCell;

use windows::core::PCWSTR;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_F11, VK_LEFT, VK_OEM_4, VK_OEM_6, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD, VK_OEM_PLUS,
    VK_RIGHT,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use llmouser_browser::automation::MenuItemInfo;
use llmouser_browser::i18n::fill;
use llmouser_browser::{Language, APP_NAME};

use super::app::{self, state};
use super::util::hs;
use super::{webview, window};

/// Command ids start here; each entry of `COMMANDS` gets `BASE + index`.
const BASE: u16 = 1000;

/// (id, accelerator as shown in the suite, virtual key, modifier flags)
const COMMANDS: &[(&str, &str, u16, u8)] = &[
    ("new-tab", "ctrl+t", b'T' as u16, FCONTROL),
    (
        "reopen-closed-tab",
        "ctrl+shift+t",
        b'T' as u16,
        FCONTROL | FSHIFT,
    ),
    ("close-tab", "ctrl+w", b'W' as u16, FCONTROL),
    ("save-pdf", "ctrl+s", b'S' as u16, FCONTROL),
    ("save-html", "ctrl+shift+s", b'S' as u16, FCONTROL | FSHIFT),
    ("settings", "ctrl+,", VK_OEM_COMMA.0, FCONTROL),
    ("about", "", 0, 0),
    ("quit", "ctrl+q", b'Q' as u16, FCONTROL),
    ("undo", "ctrl+z", b'Z' as u16, FCONTROL),
    ("redo", "ctrl+shift+z", b'Z' as u16, FCONTROL | FSHIFT),
    ("cut", "ctrl+x", b'X' as u16, FCONTROL),
    ("copy", "ctrl+c", b'C' as u16, FCONTROL),
    ("paste", "ctrl+v", b'V' as u16, FCONTROL),
    ("delete", "", 0, 0),
    ("select-all", "ctrl+a", b'A' as u16, FCONTROL),
    ("open-location", "ctrl+l", b'L' as u16, FCONTROL),
    ("reload", "ctrl+r", b'R' as u16, FCONTROL),
    ("stop", "ctrl+.", VK_OEM_PERIOD.0, FCONTROL),
    ("reset-zoom", "ctrl+0", b'0' as u16, FCONTROL),
    ("zoom-in", "ctrl+=", VK_OEM_PLUS.0, FCONTROL),
    ("zoom-out", "ctrl+-", VK_OEM_MINUS.0, FCONTROL),
    ("fullscreen", "f11", VK_F11.0, 0),
    ("back", "alt+left", VK_LEFT.0, FALT),
    ("forward", "alt+right", VK_RIGHT.0, FALT),
    ("homepage", "", 0, 0),
    ("bracket-back", "ctrl+[", VK_OEM_4.0, FCONTROL),
    ("bracket-forward", "ctrl+]", VK_OEM_6.0, FCONTROL),
];

const FCONTROL: u8 = 0x08;
const FSHIFT: u8 = 0x04;
const FALT: u8 = 0x10;
const FVIRTKEY: u8 = 0x01;

pub fn command_id(id: &str) -> Option<u16> {
    COMMANDS
        .iter()
        .position(|c| c.0 == id)
        .map(|i| BASE + i as u16)
}

fn id_of(command: u16) -> Option<&'static str> {
    COMMANDS
        .get(command.checked_sub(BASE)? as usize)
        .map(|c| c.0)
}

fn accel_text(id: &str) -> &'static str {
    COMMANDS
        .iter()
        .find(|c| c.0 == id)
        .map(|c| c.1)
        .unwrap_or("")
}

thread_local! {
    /// The installed menu bar (`HMENU` handles are plain integers; kept for describe()).
    static MENU: RefCell<Option<HMENU>> = const { RefCell::new(None) };
    /// Top-level submenu ids in order.
    static TOP_IDS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn display_accel(id: &str) -> String {
    let a = accel_text(id);
    if a.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = a
        .split('+')
        .map(|p| match p {
            "ctrl" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            other => other.to_uppercase(),
        })
        .collect();
    parts.join("+")
}

unsafe fn add(menu: HMENU, id: &str, label: &str) {
    let text = if accel_text(id).is_empty() {
        label.to_string()
    } else {
        format!("{label}\t{}", display_accel(id))
    };
    let _ = AppendMenuW(
        menu,
        MF_STRING,
        command_id(id).unwrap_or(0) as usize,
        PCWSTR(hs(&text).as_ptr()),
    );
}

unsafe fn separator(menu: HMENU) {
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
}

/// Build and install the menu bar for `language`.
pub fn install(language: Language) {
    let m = language.messages();
    let w = app::main_window();
    unsafe {
        let bar = CreateMenu().expect("menu bar");
        let file = CreatePopupMenu().expect("menu");
        add(file, "new-tab", m.menu_new_tab);
        add(file, "reopen-closed-tab", m.menu_reopen_closed_tab);
        separator(file);
        add(file, "close-tab", m.menu_close_tab);
        separator(file);
        add(file, "save-pdf", m.menu_save_pdf);
        add(file, "save-html", m.menu_save_html);
        separator(file);
        add(file, "settings", m.menu_settings);
        separator(file);
        add(file, "about", &fill(m.menu_about, &[("app", APP_NAME)]));
        add(file, "quit", m.menu_exit);

        let edit = CreatePopupMenu().expect("menu");
        add(edit, "undo", m.menu_undo);
        add(edit, "redo", m.menu_redo);
        separator(edit);
        add(edit, "cut", m.menu_cut);
        add(edit, "copy", m.menu_copy);
        add(edit, "paste", m.menu_paste);
        add(edit, "delete", m.menu_delete);
        separator(edit);
        add(edit, "select-all", m.menu_select_all);

        let view = CreatePopupMenu().expect("menu");
        add(view, "open-location", m.menu_open_location);
        add(view, "reload", m.menu_reload);
        add(view, "stop", m.menu_stop);
        separator(view);
        add(view, "reset-zoom", m.menu_reset_zoom);
        add(view, "zoom-in", m.menu_zoom_in);
        add(view, "zoom-out", m.menu_zoom_out);
        separator(view);
        add(view, "fullscreen", m.menu_toggle_fullscreen);

        let history = CreatePopupMenu().expect("menu");
        add(history, "back", m.menu_back);
        add(history, "forward", m.menu_forward);

        let help = CreatePopupMenu().expect("menu");
        add(
            help,
            "homepage",
            &fill(m.menu_homepage, &[("app", APP_NAME)]),
        );

        for (sub, label) in [
            (file, m.menu_file),
            (edit, m.menu_edit),
            (view, m.menu_view),
            (history, m.menu_history),
            (help, m.menu_help),
        ] {
            let _ = AppendMenuW(bar, MF_POPUP, sub.0 as usize, PCWSTR(hs(label).as_ptr()));
        }
        let old = MENU.with(|s| s.borrow_mut().replace(bar));
        let _ = SetMenu(w.hwnd, Some(bar));
        let _ = DrawMenuBar(w.hwnd);
        if let Some(old) = old {
            let _ = DestroyMenu(old);
        }
    }
    TOP_IDS.with(|t| *t.borrow_mut() = vec!["file", "edit", "view", "history", "help"]);
    sync_enabled();
}

/// The accelerator table for the message loop.
pub fn accelerators() -> HACCEL {
    let entries: Vec<ACCEL> = COMMANDS
        .iter()
        .enumerate()
        .filter(|(_, c)| c.2 != 0)
        .map(|(i, c)| ACCEL {
            fVirt: ACCEL_VIRT_FLAGS(c.3 | FVIRTKEY),
            key: c.2,
            cmd: BASE + i as u16,
        })
        .collect();
    unsafe { CreateAcceleratorTableW(&entries).unwrap_or_default() }
}

/// Enabled state of the items that depend on the current tab.
pub fn sync_enabled() {
    let st = state();
    let session = st.session.borrow();
    let current = app::current_tab().and_then(|t| session.browser.tab(t));
    let states = [
        ("back", current.map(|t| t.can_back()).unwrap_or(false)),
        ("forward", current.map(|t| t.can_forward()).unwrap_or(false)),
        ("stop", current.map(|t| t.is_loading()).unwrap_or(false)),
        (
            "reload",
            current
                .map(|t| !matches!(t.content, llmouser_browser::Content::Empty))
                .unwrap_or(false),
        ),
        ("reopen-closed-tab", session.browser.closed_count() > 0),
    ];
    MENU.with(|menu| {
        if let Some(menu) = *menu.borrow() {
            for (id, enabled) in states {
                if let Some(cmd) = command_id(id) {
                    let flags = if enabled { MF_ENABLED } else { MF_GRAYED };
                    unsafe {
                        let _ = EnableMenuItem(menu, cmd as u32, flags | MF_BYCOMMAND);
                    }
                }
            }
        }
    });
}

pub fn is_enabled(id: &str) -> bool {
    let Some(cmd) = command_id(id) else {
        return false;
    };
    MENU.with(|menu| match *menu.borrow() {
        Some(menu) => {
            let state = unsafe { GetMenuState(menu, cmd as u32, MF_BYCOMMAND) };
            state != u32::MAX && (state & MF_GRAYED.0) == 0
        }
        None => true,
    })
}

/// A menu command arrived (from the menu bar, an accelerator or automation).
pub fn command(cmd: u16) {
    if let Some(id) = id_of(cmd) {
        if is_enabled(id) {
            perform(id);
        }
    }
}

pub fn perform(id: &str) {
    let w = app::main_window();
    match id {
        "new-tab" => {
            let tab = state().session.borrow_mut().browser.new_tab();
            w.create_tab(tab);
            window::render(tab);
        }
        "reopen-closed-tab" => {
            let reopened = state().session.borrow_mut().browser.reopen_closed();
            if let Some(tab) = reopened {
                w.create_tab(tab);
                window::render(tab);
            }
        }
        "close-tab" => {
            if let Some(index) = w.selected_index() {
                w.close_index(index);
            }
        }
        "save-pdf" => {
            if let Some(tab) = w.current_tab() {
                webview::save_pdf(tab);
            }
        }
        "save-html" => {
            if let Some(tab) = w.current_tab() {
                webview::save_html(tab);
            }
        }
        "settings" => app::open_settings(),
        "about" => app::open_about(),
        "quit" => unsafe {
            let _ = DestroyWindow(w.hwnd);
        },
        "undo" => webview::exec_command("undo"),
        "redo" => webview::exec_command("redo"),
        "cut" => webview::exec_command("cut"),
        "copy" => webview::exec_command("copy"),
        "paste" => webview::exec_command("paste"),
        "delete" => webview::exec_command("delete"),
        "select-all" => webview::exec_command("selectAll"),
        "open-location" => w.focus_address(),
        "reload" => {
            if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().reload(tab);
                window::render(tab);
            }
        }
        "stop" => {
            if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().stop(tab);
                window::render(tab);
            }
        }
        "reset-zoom" | "zoom-in" | "zoom-out" => {
            if let Some(entry) = w.current_entry() {
                let zoom = match id {
                    "zoom-in" => (entry.zoom.get() * 1.1).min(5.0),
                    "zoom-out" => (entry.zoom.get() / 1.1).max(0.25),
                    _ => 1.0,
                };
                entry.zoom.set(zoom);
                if let Some(controller) = entry.controller.borrow().as_ref() {
                    unsafe {
                        let _ = controller.SetZoomFactor(zoom);
                    }
                }
            }
        }
        "fullscreen" => unsafe {
            let style = GetWindowLongPtrW(w.hwnd, GWL_STYLE) as u32;
            if style & WS_OVERLAPPEDWINDOW.0 != 0 {
                SetWindowLongPtrW(w.hwnd, GWL_STYLE, (style & !WS_OVERLAPPEDWINDOW.0) as isize);
                let _ = ShowWindow(w.hwnd, SW_MAXIMIZE);
            } else {
                SetWindowLongPtrW(w.hwnd, GWL_STYLE, (style | WS_OVERLAPPEDWINDOW.0) as isize);
                let _ = ShowWindow(w.hwnd, SW_RESTORE);
            }
        },
        "back" | "bracket-back" => {
            if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().back(tab);
                window::render(tab);
            }
        }
        "forward" | "bracket-forward" => {
            if let Some(tab) = w.current_tab() {
                state().session.borrow_mut().forward(tab);
                window::render(tab);
            }
        }
        "homepage" => unsafe {
            let _ = windows::Win32::UI::Shell::ShellExecuteW(
                None,
                PCWSTR(hs("open").as_ptr()),
                PCWSTR(hs(llmouser_browser::APP_HOMEPAGE).as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        },
        _ => {}
    }
}

/// The menu bar as data.
pub fn describe() -> Vec<MenuItemInfo> {
    MENU.with(|menu| {
        let Some(menu) = *menu.borrow() else {
            return Vec::new();
        };
        let ids = TOP_IDS.with(|t| t.borrow().clone());
        let mut out = Vec::new();
        unsafe {
            let count = GetMenuItemCount(Some(menu));
            for i in 0..count.max(0) {
                let label = menu_label(menu, i as u32);
                let sub = GetSubMenu(menu, i);
                let children = if sub.is_invalid() {
                    Vec::new()
                } else {
                    describe_menu(sub)
                };
                out.push(MenuItemInfo {
                    id: ids
                        .get(i as usize)
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    label,
                    enabled: true,
                    accelerator: String::new(),
                    children,
                });
            }
        }
        out
    })
}

unsafe fn menu_label(menu: HMENU, position: u32) -> String {
    let mut buf = [0u16; 256];
    let n = GetMenuStringW(menu, position, Some(&mut buf), MF_BYPOSITION);
    let text = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    text.split('\t').next().unwrap_or("").to_string()
}

unsafe fn describe_menu(menu: HMENU) -> Vec<MenuItemInfo> {
    let mut out = Vec::new();
    let count = GetMenuItemCount(Some(menu));
    for i in 0..count.max(0) {
        let cmd = GetMenuItemID(menu, i);
        if cmd == u32::MAX || cmd == 0 {
            continue; // separator
        }
        let Some(id) = id_of(cmd as u16) else {
            continue;
        };
        let state = GetMenuState(menu, i as u32, MF_BYPOSITION);
        out.push(MenuItemInfo {
            id: id.to_string(),
            label: menu_label(menu, i as u32),
            enabled: (state & MF_GRAYED.0) == 0,
            accelerator: accel_text(id).to_string(),
            children: vec![],
        });
    }
    out
}

/// Context menu items, as (id, label).
pub fn context_items() -> Vec<(String, String)> {
    let m = state().session.borrow().messages();
    vec![
        ("cut".into(), m.menu_cut.into()),
        ("copy".into(), m.menu_copy.into()),
        ("paste".into(), m.menu_paste.into()),
        ("save-pdf".into(), m.menu_save_pdf.into()),
        ("save-html".into(), m.menu_save_html.into()),
    ]
}

/// Show the context menu at screen coordinates and run the chosen command.
pub fn popup_context_menu(x: i32, y: i32) {
    let w = app::main_window();
    let items = context_items();
    unsafe {
        let menu = CreatePopupMenu().expect("popup");
        for (id, label) in &items {
            if id == "save-pdf" {
                separator(menu);
            }
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                command_id(id).unwrap_or(0) as usize,
                PCWSTR(hs(label).as_ptr()),
            );
        }
        let chosen = TrackPopupMenuEx(
            menu,
            (TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN).0,
            x,
            y,
            w.hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        if chosen.0 != 0 {
            command(chosen.0 as u16);
        }
    }
}

/// Run the command bound to a key combination, as the accelerator table would.
pub fn activate_accel(combo: &str) -> bool {
    let wanted = combo.to_lowercase();
    match COMMANDS.iter().find(|c| c.1 == wanted) {
        Some(c) => {
            if is_enabled(c.0) {
                perform(c.0);
            }
            true
        }
        None => false,
    }
}

pub fn describe_context() -> Vec<MenuItemInfo> {
    context_items()
        .into_iter()
        .map(|(id, label)| MenuItemInfo {
            id,
            label,
            enabled: true,
            accelerator: String::new(),
            children: vec![],
        })
        .collect()
}
