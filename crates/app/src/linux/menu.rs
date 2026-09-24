//! Application actions (`app.*`), the menu bar model built from them, and the
//! context menu items. Rebuilt on every language change.

use gtk::prelude::*;
use gtk::{gio, glib};
use webkit6 as webkit;
use webkit6::prelude::*;

use llmouser_browser::automation::MenuItemInfo;
use llmouser_browser::i18n::fill;
use llmouser_browser::{Language, APP_NAME};

use super::app::{self, state};
use super::{pdf, window};

/// (id, GTK accelerators) for every action; the first one is shown in menus.
const ACCELS: &[(&str, &[&str])] = &[
    ("new-tab", &["<Primary>t"]),
    ("reopen-closed-tab", &["<Primary><Shift>t"]),
    ("close-tab", &["<Primary>w"]),
    ("save-pdf", &["<Primary>s"]),
    ("save-html", &["<Primary><Shift>s"]),
    ("settings", &["<Primary>comma"]),
    ("quit", &["<Primary>q"]),
    ("undo", &["<Primary>z"]),
    ("redo", &["<Primary><Shift>z"]),
    ("cut", &["<Primary>x"]),
    ("copy", &["<Primary>c"]),
    ("paste", &["<Primary>v"]),
    ("select-all", &["<Primary>a"]),
    ("open-location", &["<Primary>l"]),
    ("reload", &["<Primary>r", "F5"]),
    ("stop", &["<Primary>period", "Escape"]),
    ("reset-zoom", &["<Primary>0"]),
    ("zoom-in", &["<Primary>equal", "<Primary>plus"]),
    ("zoom-out", &["<Primary>minus"]),
    ("fullscreen", &["F11"]),
    ("back", &["<Alt>Left", "<Primary>bracketleft"]),
    ("forward", &["<Alt>Right", "<Primary>bracketright"]),
];

const ACTIONS: &[&str] = &[
    "new-tab",
    "reopen-closed-tab",
    "close-tab",
    "save-pdf",
    "save-html",
    "settings",
    "quit",
    "about",
    "undo",
    "redo",
    "cut",
    "copy",
    "paste",
    "delete",
    "select-all",
    "open-location",
    "reload",
    "stop",
    "reset-zoom",
    "zoom-in",
    "zoom-out",
    "fullscreen",
    "back",
    "forward",
    "homepage",
];

pub fn install_actions(app: &gtk::Application) {
    for id in ACTIONS {
        let action = gio::SimpleAction::new(id, None);
        let name = id.to_string();
        action.connect_activate(move |_, _| perform(&name));
        app.add_action(&action);
    }
    for (id, accels) in ACCELS {
        app.set_accels_for_action(&format!("app.{id}"), accels);
    }
}

/// Run an action the way its menu item would.
pub fn activate(id: &str) {
    state().app.activate_action(id, None);
}

fn with_current(f: impl FnOnce(llmouser_browser::TabId)) {
    if let Some(tab) = app::current_tab() {
        f(tab);
    }
}

fn editing(command: &glib::GStr) {
    let w = app::main_window();
    if let Some(entry) = w.current_entry() {
        entry.webview.execute_editing_command(command);
    }
}

fn perform(id: &str) {
    match id {
        "new-tab" => {
            let tab = state().session.borrow_mut().browser.new_tab();
            app::main_window().create_tab(tab);
            window::render(tab);
        }
        "reopen-closed-tab" => {
            let reopened = state().session.borrow_mut().browser.reopen_closed();
            if let Some(tab) = reopened {
                app::main_window().create_tab(tab);
                window::render(tab);
            }
        }
        "close-tab" => {
            let w = app::main_window();
            if let Some(entry) = w.current_entry() {
                w.tab_view.close_page(&entry.page);
            }
        }
        "save-pdf" => with_current(pdf::save_pdf),
        "save-html" => with_current(pdf::save_html),
        "settings" => app::open_settings(),
        "quit" => state().app.quit(),
        "about" => app::open_about(),
        "undo" => editing(webkit::EDITING_COMMAND_UNDO),
        "redo" => editing(webkit::EDITING_COMMAND_REDO),
        "cut" => editing(webkit::EDITING_COMMAND_CUT),
        "copy" => editing(webkit::EDITING_COMMAND_COPY),
        "paste" => editing(webkit::EDITING_COMMAND_PASTE),
        "delete" => editing(glib::gstr!("Delete")),
        "select-all" => editing(webkit::EDITING_COMMAND_SELECT_ALL),
        "open-location" => app::main_window().focus_address(),
        "reload" => with_current(|tab| {
            state().session.borrow_mut().reload(tab);
            window::render(tab);
        }),
        "stop" => with_current(|tab| {
            state().session.borrow_mut().stop(tab);
            window::render(tab);
        }),
        "reset-zoom" | "zoom-in" | "zoom-out" => {
            let w = app::main_window();
            if let Some(entry) = w.current_entry() {
                let zoom = match id {
                    "zoom-in" => (entry.zoom.get() * 1.1).min(5.0),
                    "zoom-out" => (entry.zoom.get() / 1.1).max(0.25),
                    _ => 1.0,
                };
                entry.zoom.set(zoom);
                entry.webview.set_zoom_level(zoom);
            }
        }
        "fullscreen" => {
            let w = app::main_window();
            if w.window.is_fullscreen() {
                w.window.unfullscreen();
            } else {
                w.window.fullscreen();
            }
        }
        "back" => with_current(|tab| {
            state().session.borrow_mut().back(tab);
            window::render(tab);
        }),
        "forward" => with_current(|tab| {
            state().session.borrow_mut().forward(tab);
            window::render(tab);
        }),
        "homepage" => {
            let launcher = gtk::UriLauncher::new(llmouser_browser::APP_HOMEPAGE);
            launcher.launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |_| {});
        }
        _ => {}
    }
}

fn item(id: &str, label: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(&format!("app.{id}")));
    item.set_attribute_value("x-id", Some(&id.to_variant()));
    item
}

fn section(items: &[gio::MenuItem]) -> gio::Menu {
    let menu = gio::Menu::new();
    for i in items {
        menu.append_item(i);
    }
    menu
}

fn submenu(id: &str, label: &str, sections: &[gio::Menu]) -> gio::MenuItem {
    let menu = gio::Menu::new();
    for s in sections {
        menu.append_section(None, s);
    }
    let holder = gio::MenuItem::new(Some(label), None);
    holder.set_submenu(Some(&menu));
    holder.set_attribute_value("x-id", Some(&id.to_variant()));
    holder
}

/// Build the menu bar for `language` and install it.
pub fn install(language: Language) {
    let m = language.messages();
    let model = gio::Menu::new();
    model.append_item(&submenu(
        "file",
        m.menu_file,
        &[
            section(&[
                item("new-tab", m.menu_new_tab),
                item("reopen-closed-tab", m.menu_reopen_closed_tab),
            ]),
            section(&[item("close-tab", m.menu_close_tab)]),
            section(&[
                item("save-pdf", m.menu_save_pdf),
                item("save-html", m.menu_save_html),
            ]),
            section(&[item("settings", m.menu_settings)]),
            section(&[
                item("about", &fill(m.menu_about, &[("app", APP_NAME)])),
                item("quit", m.menu_exit),
            ]),
        ],
    ));
    model.append_item(&submenu(
        "edit",
        m.menu_edit,
        &[
            section(&[item("undo", m.menu_undo), item("redo", m.menu_redo)]),
            section(&[
                item("cut", m.menu_cut),
                item("copy", m.menu_copy),
                item("paste", m.menu_paste),
                item("delete", m.menu_delete),
            ]),
            section(&[item("select-all", m.menu_select_all)]),
        ],
    ));
    model.append_item(&submenu(
        "view",
        m.menu_view,
        &[
            section(&[
                item("open-location", m.menu_open_location),
                item("reload", m.menu_reload),
                item("stop", m.menu_stop),
            ]),
            section(&[
                item("reset-zoom", m.menu_reset_zoom),
                item("zoom-in", m.menu_zoom_in),
                item("zoom-out", m.menu_zoom_out),
            ]),
            section(&[item("fullscreen", m.menu_toggle_fullscreen)]),
        ],
    ));
    model.append_item(&submenu(
        "history",
        m.menu_history,
        &[section(&[
            item("back", m.menu_back),
            item("forward", m.menu_forward),
        ])],
    ));
    model.append_item(&submenu(
        "help",
        m.menu_help,
        &[section(&[item(
            "homepage",
            &fill(m.menu_homepage, &[("app", APP_NAME)]),
        )])],
    ));
    app::main_window().menubar.set_menu_model(Some(&model));
    sync_enabled();
}

/// Enabled state of the actions that depend on the current tab.
pub fn sync_enabled() {
    let st = state();
    let session = st.session.borrow();
    let current = app::current_tab().and_then(|t| session.browser.tab(t));
    let set = |id: &str, enabled: bool| {
        if let Some(action) = st
            .app
            .lookup_action(id)
            .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
        {
            action.set_enabled(enabled);
        }
    };
    set("back", current.map(|t| t.can_back()).unwrap_or(false));
    set("forward", current.map(|t| t.can_forward()).unwrap_or(false));
    set("stop", current.map(|t| t.is_loading()).unwrap_or(false));
    set(
        "reload",
        current
            .map(|t| !matches!(t.content, llmouser_browser::Content::Empty))
            .unwrap_or(false),
    );
    set("reopen-closed-tab", session.browser.closed_count() > 0);
}

/// Context menu items: edit actions plus the save commands, as GTK actions.
pub fn context_items() -> Vec<webkit::ContextMenuItem> {
    let st = state();
    let m = st.session.borrow().messages();
    let mut items = Vec::new();
    for (id, label) in [
        ("cut", m.menu_cut),
        ("copy", m.menu_copy),
        ("paste", m.menu_paste),
    ] {
        if let Some(action) = st.app.lookup_action(id) {
            items.push(webkit::ContextMenuItem::from_gaction(&action, label, None));
        }
    }
    items.push(webkit::ContextMenuItem::new_separator());
    for (id, label) in [
        ("save-pdf", m.menu_save_pdf),
        ("save-html", m.menu_save_html),
    ] {
        if let Some(action) = st.app.lookup_action(id) {
            items.push(webkit::ContextMenuItem::from_gaction(&action, label, None));
        }
    }
    items
}

/// The context menu as data (ids from the actions, labels as shown).
pub fn describe_context() -> Vec<MenuItemInfo> {
    let st = state();
    let m = st.session.borrow().messages();
    [
        ("cut", m.menu_cut),
        ("copy", m.menu_copy),
        ("paste", m.menu_paste),
        ("save-pdf", m.menu_save_pdf),
        ("save-html", m.menu_save_html),
    ]
    .iter()
    .map(|(id, label)| MenuItemInfo {
        id: id.to_string(),
        label: label.to_string(),
        enabled: st
            .app
            .lookup_action(id)
            .map(|a| a.is_enabled())
            .unwrap_or(false),
        accelerator: String::new(),
        children: vec![],
    })
    .collect()
}

/// The menu bar as data.
pub fn describe() -> Vec<MenuItemInfo> {
    let model = app::main_window().menubar.menu_model();
    model.map(|m| describe_model(&m)).unwrap_or_default()
}

fn describe_model(model: &gio::MenuModel) -> Vec<MenuItemInfo> {
    let st = state();
    let mut out = Vec::new();
    for i in 0..model.n_items() {
        if let Some(section) = model.item_link(i, "section") {
            out.extend(describe_model(&section));
            continue;
        }
        let id = model
            .item_attribute_value(i, "x-id", None)
            .and_then(|v| v.get::<String>())
            .unwrap_or_default();
        let label = model
            .item_attribute_value(i, "label", None)
            .and_then(|v| v.get::<String>())
            .unwrap_or_default();
        let action = model
            .item_attribute_value(i, "action", None)
            .and_then(|v| v.get::<String>());
        let enabled = match &action {
            Some(a) => st
                .app
                .lookup_action(a.trim_start_matches("app."))
                .map(|x| x.is_enabled())
                .unwrap_or(false),
            None => true,
        };
        let accelerator = action
            .as_ref()
            .and_then(|a| st.app.accels_for_action(a).first().map(|s| format_accel(s)))
            .unwrap_or_default();
        let children = model
            .item_link(i, "submenu")
            .map(|s| describe_model(&s))
            .unwrap_or_default();
        out.push(MenuItemInfo {
            id,
            label,
            enabled,
            accelerator,
            children,
        });
    }
    out
}

/// `<Primary><Shift>t` → `ctrl+shift+t`, `<Alt>Left` → `alt+left`.
pub fn format_accel(accel: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = accel;
    while let Some(end) = rest
        .strip_prefix('<')
        .and_then(|r| r.find('>').map(|e| (e, r)))
    {
        let (e, r) = end;
        let modifier = &r[..e];
        parts.push(
            match modifier {
                "Primary" | "Control" | "Ctrl" => "ctrl",
                "Shift" => "shift",
                "Alt" => "alt",
                "Super" | "Meta" => "meta",
                other => other,
            }
            .to_string(),
        );
        rest = &r[e + 1..];
    }
    let key = match rest {
        "comma" => ",",
        "period" => ".",
        "equal" => "=",
        "minus" => "-",
        "plus" => "+",
        "bracketleft" => "[",
        "bracketright" => "]",
        other => other,
    };
    parts.push(key.to_lowercase());
    parts.join("+")
}

/// Activate the action bound to a key combination, as GTK's shortcut handling would.
pub fn activate_accel(combo: &str) -> bool {
    let wanted = combo.to_lowercase();
    for (id, accels) in ACCELS {
        if accels.iter().any(|a| format_accel(a) == wanted) {
            if state()
                .app
                .lookup_action(id)
                .map(|a| a.is_enabled())
                .unwrap_or(false)
            {
                activate(id);
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::format_accel;

    #[test]
    fn accelerators_format_like_the_suite_expects() {
        assert_eq!(format_accel("<Primary>t"), "ctrl+t");
        assert_eq!(format_accel("<Primary><Shift>t"), "ctrl+shift+t");
        assert_eq!(format_accel("<Alt>Left"), "alt+left");
        assert_eq!(format_accel("<Primary>comma"), "ctrl+,");
        assert_eq!(format_accel("F11"), "f11");
    }
}
