//! Answers automation commands by operating the real Win32 controls.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::WindowsAndMessaging::{DestroyWindow, SendMessageW, BM_CLICK};

use llmouser_browser::automation::{
    self, AboutInfo, Command, Response, SettingsInfo, StateInfo, TabInfo,
};

use super::app::{self, on_main, state};
use super::util::get_text;
use super::{menu, webview, window};

pub fn start(port: u16) -> std::io::Result<()> {
    automation::serve(
        port,
        Arc::new(|command, reply| on_main(move || handle(command, reply))),
    )
}

fn handle(command: Command, reply: Sender<Response>) {
    let response = match command {
        Command::Eval { js } => {
            eval(js, reply);
            return;
        }
        Command::Quit => {
            let _ = reply.send(Response::unit());
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(150));
                on_main(|| unsafe {
                    let _ = DestroyWindow(app::main_window().hwnd);
                });
            });
            return;
        }
        other => dispatch(other),
    };
    let _ = reply.send(response);
}

fn click(hwnd: windows::Win32::Foundation::HWND) -> Response {
    unsafe {
        if !IsWindowEnabled(hwnd).as_bool() {
            return Response::err("button is disabled");
        }
        SendMessageW(hwnd, BM_CLICK, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    Response::unit()
}

fn dispatch(command: Command) -> Response {
    let w = app::main_window();
    match command {
        Command::Click { target } => match target.as_str() {
            "back" => click(w.back),
            "forward" => click(w.forward),
            "go" | "stop" => click(w.go),
            "settings" => click(w.settings_button),
            "new_tab" => click(w.new_tab_button),
            other => Response::err(format!("unknown click target {other}")),
        },
        Command::SetAddress { value } => {
            super::util::set_text(w.address, &value);
            Response::unit()
        }
        Command::SubmitAddress => {
            window::submit_address();
            Response::unit()
        }
        Command::State => Response::ok(state_info()),
        Command::ActivateTab { index } => {
            if index < w.tabs.borrow().len() {
                w.select_index(index);
                Response::unit()
            } else {
                Response::err(format!("no tab at index {index}"))
            }
        }
        Command::CloseTab { index } => {
            if index < w.tabs.borrow().len() {
                w.select_index(index);
                click(w.close_tab_button)
            } else {
                Response::err(format!("no tab at index {index}"))
            }
        }
        Command::Menu => Response::ok(menu::describe()),
        Command::MenuClick { id } => match menu::command_id(&id) {
            Some(cmd) if menu::is_enabled(&id) => {
                menu::command(cmd);
                Response::unit()
            }
            Some(_) => Response::err(format!("menu item {id} is disabled")),
            None => Response::err(format!("no menu item {id}")),
        },
        Command::ContextMenu { .. } => {
            *state().context_menu_open.borrow_mut() = menu::context_items();
            Response::ok(menu::describe_context())
        }
        Command::ContextMenuClick { id } => {
            let known = state()
                .context_menu_open
                .borrow()
                .iter()
                .any(|(i, _)| *i == id);
            if known {
                menu::perform(&id);
                Response::unit()
            } else {
                Response::err(format!("no context item {id}"))
            }
        }
        Command::Settings => {
            let s = state().settings.borrow().clone();
            Response::ok(s.map(|s| s.info()).unwrap_or(SettingsInfo {
                open: false,
                ..SettingsInfo::default()
            }))
        }
        Command::SettingsSet { field, value } => {
            with_settings(|s| match s.set_field(&field, &value) {
                Ok(()) => Response::unit(),
                Err(e) => Response::err(e),
            })
        }
        Command::SettingsTab { tab } => with_settings(|s| {
            s.select_tab(&tab);
            Response::unit()
        }),
        Command::SettingsSave => with_settings(|s| click(s.save)),
        Command::SettingsClose => with_settings(|s| click(s.close)),
        Command::About => {
            let a = state().about.borrow().clone();
            Response::ok(a.map(|a| a.info()).unwrap_or(AboutInfo {
                open: false,
                ..AboutInfo::default()
            }))
        }
        Command::AboutClose => {
            let a = state().about.borrow().clone();
            match a {
                Some(a) if a.is_open() => {
                    a.close();
                    Response::unit()
                }
                _ => Response::err("about window is not open"),
            }
        }
        Command::Key { combo } => {
            if menu::activate_accel(&combo) {
                Response::unit()
            } else {
                Response::err(format!("no shortcut for {combo}"))
            }
        }
        Command::Eval { .. } | Command::Quit => Response::err("handled elsewhere"),
    }
}

fn with_settings(f: impl FnOnce(&super::settings::SettingsWindow) -> Response) -> Response {
    let s = state().settings.borrow().clone();
    match s {
        Some(s) if s.is_open() => f(&s),
        _ => Response::err("settings window is not open"),
    }
}

fn state_info() -> StateInfo {
    let st = state();
    let w = app::main_window();
    let selected = w.selected_index().unwrap_or(0);
    let settings_open = st
        .settings
        .borrow()
        .as_ref()
        .map(|s| s.is_open())
        .unwrap_or(false);
    let about_open = st
        .about
        .borrow()
        .as_ref()
        .map(|a| a.is_open())
        .unwrap_or(false);
    let session = st.session.borrow();
    let tabs: Vec<TabInfo> = w
        .tabs
        .borrow()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let t = session.browser.tab(e.tab);
            TabInfo {
                title: t
                    .map(|t| t.display_title(session.messages()))
                    .unwrap_or_default(),
                active: i == selected,
                icon: match t {
                    Some(t) if t.icon.is_some() => "favicon",
                    Some(t) if !t.url.is_empty() => "letter",
                    Some(_) => "blank",
                    None => "none",
                }
                .into(),
                loading: t.map(|t| t.is_loading()).unwrap_or(false),
            }
        })
        .collect();
    let current = w.current_tab().and_then(|t| session.browser.tab(t));
    let address_placeholder = w.placeholder.borrow().clone();
    let tooltips: Vec<String> = w.tooltip_texts.borrow().iter().take(4).cloned().collect();
    StateInfo {
        address: get_text(w.address),
        status: get_text(w.status),
        status_error: w.status_error.get(),
        content_seq: current.map(|t| t.content_seq).unwrap_or(0),
        can_back: unsafe { IsWindowEnabled(w.back).as_bool() },
        can_forward: unsafe { IsWindowEnabled(w.forward).as_bool() },
        loading: current.map(|t| t.is_loading()).unwrap_or(false),
        tabs,
        active_tab: selected,
        settings_open,
        about_open,
        window_title: get_text(w.hwnd),
        address_placeholder,
        tooltips,
    }
}

fn eval(js: String, reply: Sender<Response>) {
    let w = app::main_window();
    let Some(entry) = w.current_entry() else {
        let _ = reply.send(Response::err("no browser window"));
        return;
    };
    let wrapped = format!(
        "(function(){{ try {{ return JSON.stringify({{v: (function(){{ return (\n{js}\n); }})()}}) }} \
         catch (e) {{ return JSON.stringify({{e: String(e)}}) }} }})()"
    );
    webview::eval(&entry, &wrapped, move |result| {
        let response = match result {
            Err(e) => Response::err(e.to_string()),
            Ok(json) => {
                // ExecuteScript returns the JSON encoding of the (string) result.
                let text: String = serde_json::from_str(&json).unwrap_or_default();
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(v) if v.get("e").is_some() => {
                        Response::err(v["e"].as_str().unwrap_or("error").to_string())
                    }
                    Ok(v) => Response::ok(v.get("v").cloned().unwrap_or(serde_json::Value::Null)),
                    Err(_) => Response::ok(serde_json::Value::Null),
                }
            }
        };
        let _ = reply.send(response);
    });
}
