//! Answers automation commands by operating the real widgets: toolbar items
//! are sent their actions, fields are typed into, menus are performed.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadOnly};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSMenu, NSToolbarItem, NSWindow};
use objc2_foundation::{NSError, NSPoint, NSString};

use llmouser_browser::automation::{
    self, AboutInfo, Command, Response, SettingsInfo, StateInfo, TabInfo,
};

use super::app::{self, state};
use super::menu;
use super::util::{ns, on_main};
use super::window::TabWindow;

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
                on_main(|| app::app(state().mtm).terminate(None));
            });
            return;
        }
        other => dispatch(other),
    };
    let _ = reply.send(response);
}

fn dispatch(command: Command) -> Response {
    match command {
        Command::Click { target } => click(&target),
        Command::SetAddress { value } => with_window(|w| {
            w.address.setStringValue(&ns(&value));
            app::on_address_typed(w.tab, &value);
            Response::unit()
        }),
        Command::SubmitAddress => with_window(|w| {
            let st = state();
            unsafe {
                app::app(st.mtm).sendAction_to_from(
                    sel!(navigate:),
                    Some(st.delegate.as_target()),
                    Some(&w.address),
                )
            };
            Response::unit()
        }),
        Command::State => Response::ok(state_info()),
        Command::ActivateTab { index } => with_window(|w| match tab_windows(w).get(index) {
            Some(win) => {
                if let Some(group) = win.tabGroup() {
                    group.setSelectedWindow(Some(win));
                }
                win.makeKeyAndOrderFront(None);
                let tab = state()
                    .windows
                    .borrow()
                    .iter()
                    .find(|t| std::ptr::eq(&*t.window, &**win))
                    .map(|t| t.tab);
                if let Some(tab) = tab {
                    app::set_current(tab);
                }
                Response::unit()
            }
            None => Response::err(format!("no tab at index {index}")),
        }),
        Command::CloseTab { index } => with_window(|w| match tab_windows(w).get(index) {
            Some(win) => {
                win.performClose(None);
                Response::unit()
            }
            None => Response::err(format!("no tab at index {index}")),
        }),
        Command::Menu => {
            let main = app::app(state().mtm).mainMenu();
            Response::ok(main.map(|m| menu::describe(&m)).unwrap_or_default())
        }
        Command::MenuClick { id } => {
            let Some(main) = app::app(state().mtm).mainMenu() else {
                return Response::err("no menu");
            };
            match menu::find_item(&main, &id) {
                Some((menu, item)) => {
                    menu.update();
                    if !item.isEnabled() {
                        return Response::err(format!("menu item {id} is disabled"));
                    }
                    menu.performActionForItemAtIndex(menu.indexOfItem(&item));
                    Response::unit()
                }
                None => Response::err(format!("no menu item {id}")),
            }
        }
        Command::ContextMenu { x, y } => with_window(|w| {
            let st = state();
            w.focus_page();
            let menu = NSMenu::initWithTitle(NSMenu::alloc(st.mtm), &ns("context"));
            let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::RightMouseDown,
                NSPoint::new(x, y),
                NSEventModifierFlags::empty(),
                0.0,
                w.window.windowNumber(),
                None,
                0,
                1,
                0.0,
            );
            match event {
                Some(event) => {
                    w.webview.willOpenMenu_withEvent(&menu, &event);
                    let items = menu::describe(&menu);
                    *st.context_menu.borrow_mut() = Some(menu);
                    Response::ok(items)
                }
                None => Response::err("could not synthesize a mouse event"),
            }
        }),
        Command::ContextMenuClick { id } => {
            let menu = state().context_menu.borrow().clone();
            let Some(menu) = menu else {
                return Response::err("no context menu is open");
            };
            match menu::find_item(&menu, &id) {
                Some((menu, item)) => {
                    menu.update();
                    if !item.isEnabled() {
                        return Response::err(format!("context item {id} is disabled"));
                    }
                    menu.performActionForItemAtIndex(menu.indexOfItem(&item));
                    Response::unit()
                }
                None => Response::err(format!("no context item {id}")),
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
        Command::SettingsSave => with_settings(|s| {
            unsafe { s.save.performClick(None) };
            Response::unit()
        }),
        Command::SettingsClose => with_settings(|s| {
            unsafe { s.close.performClick(None) };
            Response::unit()
        }),
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
                Some(a) => {
                    a.close();
                    Response::unit()
                }
                None => Response::err("about window is not open"),
            }
        }
        Command::Key { combo } => key(&combo),
        Command::Eval { .. } | Command::Quit => Response::err("handled elsewhere"),
    }
}

fn with_window(f: impl FnOnce(&TabWindow) -> Response) -> Response {
    match app::current_window() {
        Some(w) => f(&w),
        None => Response::err("no browser window"),
    }
}

fn with_settings(f: impl FnOnce(&super::settings::SettingsWindow) -> Response) -> Response {
    let s = state().settings.borrow().clone();
    match s {
        Some(s) if s.is_open() => f(&s),
        _ => Response::err("settings window is not open"),
    }
}

fn send_item(item: &NSToolbarItem) -> Response {
    if !item.isEnabled() {
        return Response::err("toolbar item is disabled");
    }
    let st = state();
    let target = item.target();
    let sent = match item.action() {
        Some(action) => unsafe {
            app::app(st.mtm).sendAction_to_from(action, target.as_deref(), Some(item))
        },
        None => false,
    };
    if sent {
        Response::unit()
    } else {
        Response::err("action was not delivered")
    }
}

fn click(target: &str) -> Response {
    if target == "new_tab" {
        let st = state();
        let sender: Option<Retained<NSWindow>> = app::current_window().map(|w| w.window.clone());
        unsafe {
            app::app(st.mtm).sendAction_to_from(
                sel!(newTab:),
                Some(st.delegate.as_target()),
                sender.as_deref().map(|w| w as &AnyObject),
            )
        };
        return Response::unit();
    }
    with_window(|w| match target {
        "back" => send_item(&w.back_item),
        "forward" => send_item(&w.forward_item),
        "go" | "stop" => send_item(&w.go_item),
        "settings" => send_item(&w.settings_item),
        other => Response::err(format!("unknown click target {other}")),
    })
}

/// Windows of the current tab group, in tab-bar order.
fn tab_windows(w: &TabWindow) -> Vec<Retained<NSWindow>> {
    w.window
        .tabGroup()
        .map(|g| g.windows().to_vec())
        .unwrap_or_else(|| vec![w.window.clone()])
}

fn state_info() -> StateInfo {
    let st = state();
    let Some(w) = app::current_window() else {
        return StateInfo {
            settings_open: settings_open(),
            about_open: about_open(),
            ..StateInfo::default()
        };
    };
    let windows = st.windows.borrow();
    let mut tabs = Vec::new();
    let mut active_tab = 0;
    for (i, win) in tab_windows(&w).iter().enumerate() {
        let tw = windows.iter().find(|t| std::ptr::eq(&*t.window, &**win));
        let active = std::ptr::eq(&**win, &*w.window);
        if active {
            active_tab = i;
        }
        tabs.push(TabInfo {
            title: win.title().to_string(),
            active,
            icon: "none".into(),
            loading: tw.map(|t| t.is_loading()).unwrap_or(false),
        });
    }
    let session = st.session.borrow();
    let tab = session.browser.tab(w.tab);
    StateInfo {
        address: w.address.stringValue().to_string(),
        status: w.status.stringValue().to_string(),
        status_error: tab.map(|t| t.status.is_error()).unwrap_or(false),
        content_seq: tab.map(|t| t.content_seq).unwrap_or(0),
        can_back: w.back_item.isEnabled(),
        can_forward: w.forward_item.isEnabled(),
        loading: w.is_loading(),
        tabs,
        active_tab,
        settings_open: settings_open(),
        about_open: about_open(),
        window_title: w.window.title().to_string(),
        address_placeholder: w
            .address
            .placeholderString()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        tooltips: [&w.back_item, &w.forward_item, &w.go_item, &w.settings_item]
            .iter()
            .map(|i| i.toolTip().map(|s| s.to_string()).unwrap_or_default())
            .collect(),
    }
}

fn settings_open() -> bool {
    state()
        .settings
        .borrow()
        .as_ref()
        .map(|s| s.is_open())
        .unwrap_or(false)
}

fn about_open() -> bool {
    state()
        .about
        .borrow()
        .as_ref()
        .map(|a| a.window.isVisible())
        .unwrap_or(false)
}

fn eval(js: String, reply: Sender<Response>) {
    let Some(w) = app::current_window() else {
        let _ = reply.send(Response::err("no browser window"));
        return;
    };
    // No eval(): generated pages forbid it through their CSP. The expression is
    // inlined as the body of a function instead.
    let wrapped = format!(
        "(function(){{ try {{ return JSON.stringify({{v: (function(){{ return (\n{js}\n); }})()}}) }} \
         catch (e) {{ return JSON.stringify({{e: String(e)}}) }} }})()"
    );
    let block = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
        let response = if let Some(error) = unsafe { error.as_ref() } {
            Response::err(error.localizedDescription().to_string())
        } else {
            let text = unsafe { result.as_ref() }
                .and_then(|o| o.downcast_ref::<NSString>())
                .map(|s| s.to_string())
                .unwrap_or_default();
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) if v.get("e").is_some() => {
                    Response::err(v["e"].as_str().unwrap_or("error").to_string())
                }
                Ok(v) => Response::ok(v.get("v").cloned().unwrap_or(serde_json::Value::Null)),
                Err(_) => Response::ok(serde_json::Value::Null),
            }
        };
        let _ = reply.send(response);
    });
    unsafe {
        w.webview
            .evaluateJavaScript_completionHandler(&ns(&wrapped), Some(&block))
    };
}

/// Press a key combination such as `cmd+shift+t`, `cmd+[`, `escape`, `enter`.
fn key(combo: &str) -> Response {
    let mut flags = NSEventModifierFlags::empty();
    let mut key = String::new();
    for part in combo.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "cmd" | "meta" | "command" => flags |= NSEventModifierFlags::Command,
            "shift" => flags |= NSEventModifierFlags::Shift,
            "alt" | "option" => flags |= NSEventModifierFlags::Option,
            "ctrl" | "control" => flags |= NSEventModifierFlags::Control,
            other => key = other.to_string(),
        }
    }
    let (chars, code): (String, u16) = match key.as_str() {
        "escape" | "esc" => ("\u{1b}".into(), 53),
        "enter" | "return" => ("\r".into(), 36),
        "tab" => ("\t".into(), 48),
        "[" => ("[".into(), 33),
        "]" => ("]".into(), 30),
        other => (other.to_string(), 0),
    };
    let st = state();
    let Some(w) = app::current_window() else {
        return Response::err("no browser window");
    };
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        flags,
        0.0,
        w.window.windowNumber(),
        None,
        &ns(&chars),
        &ns(&chars),
        false,
        code,
    );
    match event {
        Some(event) => {
            app::app(st.mtm).sendEvent(&event);
            Response::unit()
        }
        None => Response::err("could not synthesize a key event"),
    }
}
