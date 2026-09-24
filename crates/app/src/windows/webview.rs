//! WebView2: environment creation, per-tab controllers, navigation policy,
//! network blocking, context menu, script evaluation and PDF export.

use std::path::PathBuf;
use std::rc::Rc;

use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler, ContextMenuRequestedEventHandler,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    ExecuteScriptCompletedHandler, NavigationStartingEventHandler, NewWindowRequestedEventHandler,
    PrintToPdfCompletedHandler, WebResourceRequestedEventHandler,
};
use windows::core::{w, Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};

use llmouser_browser::page::CHROME_SCHEME;
use llmouser_browser::{Status, TabId};

use super::app::{self, state};
use super::menu;
use super::util::hs;
use super::window::{self, TabEntry};
use crate::session::{default_file_name, Session};

pub fn create_environment() {
    let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(|error, env| {
        match env {
            Some(env) => {
                *state().env.borrow_mut() = Some(env);
                app::main_window().ensure_webviews();
            }
            None => eprintln!("WebView2 runtime unavailable: {error:?}"),
        }
        Ok(())
    }));
    let data_dir = state()
        .session
        .borrow()
        .store
        .path()
        .parent()
        .map(|p| p.join("webview2"))
        .unwrap_or_default();
    unsafe {
        if let Err(e) = CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            PCWSTR(hs(&data_dir.display().to_string()).as_ptr()),
            None,
            &handler,
        ) {
            eprintln!("WebView2 environment failed: {e}");
        }
    }
}

pub fn create(env: &ICoreWebView2Environment, parent: HWND, entry: Rc<TabEntry>) {
    let tab = entry.tab;
    let env_for_requests = env.clone();
    let handler =
        CreateCoreWebView2ControllerCompletedHandler::create(Box::new(move |error, controller| {
            let Some(controller) = controller else {
                eprintln!("WebView2 controller failed: {error:?}");
                return Ok(());
            };
            unsafe {
                let webview = controller.CoreWebView2()?;
                let settings = webview.Settings()?;
                let _ = settings.SetIsStatusBarEnabled(false);
                let _ = settings.SetAreDevToolsEnabled(false);
                let _ = settings.SetIsZoomControlEnabled(false);
                let _ = settings.SetAreDefaultContextMenusEnabled(true);

                let mut token: i64 = 0;
                webview.add_NavigationStarting(
                    &NavigationStartingEventHandler::create(Box::new(move |_wv, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let uri = take_pwstr(uri);
                        if !decide(tab, &uri) {
                            args.SetCancel(true)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                webview.add_FrameNavigationStarting(
                    &NavigationStartingEventHandler::create(Box::new(|_wv, args| {
                        if let Some(args) = args {
                            args.SetCancel(true)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                webview.add_NewWindowRequested(
                    &NewWindowRequestedEventHandler::create(Box::new(move |_wv, args| {
                        let Some(args) = args else { return Ok(()) };
                        args.SetHandled(true)?;
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let uri = take_pwstr(uri);
                        state().session.borrow_mut().open_link(tab, &uri);
                        window::render(tab);
                        Ok(())
                    })),
                    &mut token,
                )?;
                // Nothing from the real network, in any frame.
                webview.AddWebResourceRequestedFilter(
                    w!("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                )?;
                let env_inner = env_for_requests.clone();
                webview.add_WebResourceRequested(
                    &WebResourceRequestedEventHandler::create(Box::new(move |_wv, args| {
                        let Some(args) = args else { return Ok(()) };
                        let request = args.Request()?;
                        let mut uri = PWSTR::null();
                        request.Uri(&mut uri)?;
                        let uri = take_pwstr(uri).to_ascii_lowercase();
                        if uri.starts_with("http://")
                            || uri.starts_with("https://")
                            || uri.starts_with("ws://")
                            || uri.starts_with("wss://")
                            || uri.starts_with("ftp://")
                        {
                            let response = env_inner.CreateWebResourceResponse(
                                None,
                                403,
                                w!("Forbidden"),
                                w!(""),
                            )?;
                            args.SetResponse(&response)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                if let Ok(wv11) = webview.cast::<ICoreWebView2_11>() {
                    wv11.add_ContextMenuRequested(
                        &ContextMenuRequestedEventHandler::create(Box::new(move |_wv, args| {
                            let Some(args) = args else { return Ok(()) };
                            args.SetHandled(true)?;
                            let mut point = POINT::default();
                            args.Location(&mut point)?;
                            let w = app::main_window();
                            let _ = ClientToScreen(w.hwnd, &mut point);
                            menu::popup_context_menu(point.x, point.y);
                            Ok(())
                        })),
                        &mut token,
                    )?;
                }
                controller.add_AcceleratorKeyPressed(
                    &AcceleratorKeyPressedEventHandler::create(Box::new(|_controller, args| {
                        let Some(args) = args else { return Ok(()) };
                        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                        args.KeyEventKind(&mut kind)?;
                        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                            && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                        {
                            return Ok(());
                        }
                        let mut key = 0u32;
                        args.VirtualKey(&mut key)?;
                        if let Some(combo) = key_combo(key) {
                            if menu::activate_accel(&combo) {
                                args.SetHandled(true)?;
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;

                *entry.controller.borrow_mut() = Some(controller.clone());
                *entry.webview.borrow_mut() = Some(webview.clone());
                let pending = entry.pending.borrow_mut().take();
                if let Some(html) = pending {
                    window::load_document(&entry, html);
                }
                app::main_window().layout();
            }
            Ok(())
        }));
    unsafe {
        if let Err(e) = env.CreateCoreWebView2Controller(parent, &handler) {
            eprintln!("WebView2 controller creation failed: {e}");
        }
    }
}

/// Key combination for a virtual key with the current modifier state.
fn key_combo(key: u32) -> Option<String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};
    let down = |vk: u16| unsafe { GetKeyState(vk as i32) } < 0;
    let mut parts = Vec::new();
    if down(VK_CONTROL.0) {
        parts.push("ctrl");
    }
    if down(VK_MENU.0) {
        parts.push("alt");
    }
    if down(VK_SHIFT.0) {
        parts.push("shift");
    }
    let name = match key {
        0x25 => "left".to_string(),
        0x27 => "right".to_string(),
        0x7A => "f11".to_string(),
        0xBC => ",".to_string(),
        0xBE => ".".to_string(),
        0xBB => "=".to_string(),
        0xBD => "-".to_string(),
        0xDB => "[".to_string(),
        0xDD => "]".to_string(),
        k if (0x30..=0x5A).contains(&k) => (k as u8 as char).to_ascii_lowercase().to_string(),
        _ => return None,
    };
    if parts.is_empty() && name != "f11" {
        return None;
    }
    parts.push(&name);
    Some(parts.join("+"))
}

/// Whether the navigation may proceed. Our own `NavigateToString` loads report
/// `about:blank`/`data:`; everything else is regenerated through the LLM.
fn decide(tab: TabId, url: &str) -> bool {
    if let Some(rest) = url.strip_prefix(&format!("{CHROME_SCHEME}://")) {
        chrome_action(tab, rest.trim_end_matches('/'));
        return false;
    }
    if url.is_empty() || url.starts_with("about:") || url.starts_with("data:") {
        return true;
    }
    let current = state()
        .session
        .borrow()
        .browser
        .tab(tab)
        .map(|t| t.url.clone())
        .unwrap_or_default();
    if same_document(&current, url) {
        return true;
    }
    state().session.borrow_mut().open_link(tab, url);
    window::render(tab);
    false
}

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

/// Evaluate JavaScript in the current tab; `done` receives the JSON result.
type EvalResult = std::result::Result<String, String>;

pub fn eval(entry: &TabEntry, js: &str, done: impl FnOnce(EvalResult) + 'static) {
    let webview = entry.webview.borrow().clone();
    let Some(webview) = webview else {
        done(Err("webview not ready".to_string()));
        return;
    };
    let cell = std::cell::RefCell::new(Some(Box::new(done) as Box<dyn FnOnce(EvalResult)>));
    let handler = ExecuteScriptCompletedHandler::create(Box::new(move |error, result| {
        if let Some(done) = cell.borrow_mut().take() {
            if error.is_err() {
                done(Err(format!("{error:?}")));
            } else {
                done(Ok(result));
            }
        }
        Ok(())
    }));
    unsafe {
        if let Err(e) = webview.ExecuteScript(PCWSTR(hs(js).as_ptr()), &handler) {
            eprintln!("ExecuteScript failed: {e}");
        }
    }
}

/// `document.execCommand` in the page (edit menu and context menu).
pub fn exec_command(command: &str) {
    if let Some(entry) = app::main_window().current_entry() {
        let js = format!("document.execCommand({command:?})");
        eval(&entry, &js, |_| {});
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Pdf,
    Html,
}

pub fn save_pdf(tab: TabId) {
    save(tab, Kind::Pdf);
}

pub fn save_html(tab: TabId) {
    save(tab, Kind::Html);
}

fn save(tab: TabId, kind: Kind) {
    let page = state().session.borrow_mut().page_to_save(tab);
    let Some((url, _)) = page else {
        window::render(tab);
        return;
    };
    let (ext, cancelled) = match kind {
        Kind::Pdf => ("pdf", Status::PdfCanceled),
        Kind::Html => ("html", Status::HtmlCanceled),
    };
    let suggested = default_file_name(&url, ext);
    let chosen = match Session::save_dialog_stub() {
        Some(choice) => choice,
        None => save_dialog(&suggested, ext),
    };
    match chosen {
        Some(path) => write(tab, kind, path),
        None => finish(tab, cancelled),
    }
}

fn save_dialog(suggested: &str, ext: &str) -> Option<PathBuf> {
    let w = app::main_window();
    let mut file: Vec<u16> = suggested.encode_utf16().collect();
    file.resize(1024, 0);
    let filter: Vec<u16> = format!("{}\0*.{ext}\0\0", ext.to_uppercase())
        .encode_utf16()
        .collect();
    let default_ext: Vec<u16> = ext.encode_utf16().chain(Some(0)).collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: w.hwnd,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrDefExt: PCWSTR(default_ext.as_ptr()),
        Flags: OFN_OVERWRITEPROMPT,
        ..Default::default()
    };
    let ok = unsafe { GetSaveFileNameW(&mut ofn) }.as_bool();
    if !ok {
        return None;
    }
    let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
}

fn write(tab: TabId, kind: Kind, path: PathBuf) {
    match kind {
        Kind::Html => {
            let page = state()
                .session
                .borrow()
                .browser
                .tab(tab)
                .and_then(|t| t.page_html().map(|(u, h)| (u.to_string(), h.to_string())));
            let status = match page {
                Some((url, html)) => match Session::write_html(&path, &url, &html) {
                    Ok(()) => Status::SavedHtml {
                        path: path.display().to_string(),
                    },
                    Err(e) => Status::FailedHtml {
                        error: e.to_string(),
                    },
                },
                None => Status::NothingToSave,
            };
            finish(tab, status);
        }
        Kind::Pdf => {
            let w = app::main_window();
            let Some(entry) = w.entry_for(tab) else {
                return;
            };
            let webview = entry.webview.borrow().clone();
            let Some(webview) = webview else {
                finish(
                    tab,
                    Status::FailedPdf {
                        error: "webview not ready".into(),
                    },
                );
                return;
            };
            finish(tab, Status::SavingPdf);
            let done_path = path.clone();
            let handler = PrintToPdfCompletedHandler::create(Box::new(move |_error, ok| {
                let written = ok && done_path.exists();
                finish(
                    tab,
                    if written {
                        Status::SavedPdf {
                            path: done_path.display().to_string(),
                        }
                    } else {
                        Status::FailedPdf {
                            error: "print to PDF failed".into(),
                        }
                    },
                );
                Ok(())
            }));
            unsafe {
                match webview.cast::<ICoreWebView2_7>() {
                    Ok(wv7) => {
                        if let Err(e) = wv7.PrintToPdf(
                            PCWSTR(hs(&path.display().to_string()).as_ptr()),
                            None,
                            &handler,
                        ) {
                            finish(
                                tab,
                                Status::FailedPdf {
                                    error: e.to_string(),
                                },
                            );
                        }
                    }
                    Err(e) => finish(
                        tab,
                        Status::FailedPdf {
                            error: e.to_string(),
                        },
                    ),
                }
            }
        }
    }
}

fn finish(tab: TabId, status: Status) {
    state().session.borrow_mut().set_status(tab, status);
    window::render(tab);
}

/// Unused helper kept for parity with other shells' layout code.
#[allow(dead_code)]
pub fn empty_rect() -> RECT {
    RECT::default()
}
