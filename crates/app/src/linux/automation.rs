//! Answers automation commands by operating the real GTK widgets.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use gtk::prelude::*;

use llmouser_browser::automation::{
    self, AboutInfo, Command, Response, SettingsInfo, StateInfo, TabInfo,
};

use super::app::{self, on_main, state};
use super::{menu, webview};

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
            gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(150), || {
                state().app.quit()
            });
            return;
        }
        other => dispatch(other),
    };
    let _ = reply.send(response);
}

fn dispatch(command: Command) -> Response {
    let w = app::main_window();
    match command {
        Command::Click { target } => match target.as_str() {
            "back" => click(&w.back),
            "forward" => click(&w.forward),
            "go" | "stop" => click(&w.go),
            "settings" => click(&w.settings_button),
            "new_tab" => click(&w.new_tab_button),
            other => Response::err(format!("unknown click target {other}")),
        },
        Command::SetAddress { value } => {
            w.address.set_text(&value);
            Response::unit()
        }
        Command::SubmitAddress => {
            w.address.emit_activate();
            Response::unit()
        }
        Command::State => Response::ok(state_info()),
        Command::ActivateTab { index } => match w.ordered_entries().get(index) {
            Some(entry) => {
                w.tab_view.set_selected_page(&entry.page);
                Response::unit()
            }
            None => Response::err(format!("no tab at index {index}")),
        },
        Command::CloseTab { index } => match w.ordered_entries().get(index) {
            Some(entry) => {
                w.tab_view.close_page(&entry.page);
                Response::unit()
            }
            None => Response::err(format!("no tab at index {index}")),
        },
        Command::Menu => Response::ok(menu::describe()),
        Command::MenuClick { id } => match state().app.lookup_action(&id) {
            Some(action) if action.is_enabled() => {
                menu::activate(&id);
                Response::unit()
            }
            Some(_) => Response::err(format!("menu item {id} is disabled")),
            None => Response::err(format!("no menu item {id}")),
        },
        Command::ContextMenu { .. } => {
            // Build the items exactly as the context-menu signal handler does.
            let _items = menu::context_items();
            Response::ok(menu::describe_context())
        }
        Command::ContextMenuClick { id } => match state().app.lookup_action(&id) {
            Some(action) if action.is_enabled() => {
                menu::activate(&id);
                Response::unit()
            }
            _ => Response::err(format!("no context item {id}")),
        },
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
        Command::SettingsSave => with_settings(|s| click(&s.save)),
        Command::SettingsClose => with_settings(|s| click(&s.close)),
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

fn click(button: &gtk::Button) -> Response {
    if !button.is_sensitive() {
        return Response::err("button is disabled");
    }
    button.emit_clicked();
    Response::unit()
}

fn with_settings(f: impl FnOnce(&super::settings::SettingsDialog) -> Response) -> Response {
    let s = state().settings.borrow().clone();
    match s {
        Some(s) if s.is_open() => f(&s),
        _ => Response::err("settings window is not open"),
    }
}

fn state_info() -> StateInfo {
    let st = state();
    let w = app::main_window();
    let entries = w.ordered_entries();
    let mut active_tab = 0;
    let tabs: Vec<TabInfo> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            if e.page.is_selected() {
                active_tab = i;
            }
            TabInfo {
                title: e.page.title().to_string(),
                active: e.page.is_selected(),
                icon: e.icon_kind.get().to_string(),
                loading: e.page.is_loading(),
            }
        })
        .collect();
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
    let current = w.current_tab().and_then(|t| session.browser.tab(t));
    StateInfo {
        address: w.address.text().to_string(),
        status: w.status.text().to_string(),
        status_error: current.map(|t| t.status.is_error()).unwrap_or(false),
        can_back: w.back.is_sensitive(),
        can_forward: w.forward.is_sensitive(),
        loading: current.map(|t| t.is_loading()).unwrap_or(false),
        tabs,
        active_tab,
        settings_open,
        about_open,
        window_title: w.window.title().map(|t| t.to_string()).unwrap_or_default(),
        address_placeholder: w
            .address
            .placeholder_text()
            .map(|t| t.to_string())
            .unwrap_or_default(),
        tooltips: [&w.back, &w.forward, &w.go, &w.settings_button]
            .iter()
            .map(|b| b.tooltip_text().map(|t| t.to_string()).unwrap_or_default())
            .collect(),
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
    webview::eval(&entry.webview, &wrapped, move |result| {
        let response = match result {
            Err(e) => Response::err(e),
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) if v.get("e").is_some() => {
                    Response::err(v["e"].as_str().unwrap_or("error").to_string())
                }
                Ok(v) => Response::ok(v.get("v").cloned().unwrap_or(serde_json::Value::Null)),
                Err(_) => Response::ok(serde_json::Value::Null),
            },
        };
        let _ = reply.send(response);
    });
}
