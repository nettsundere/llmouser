//! The application menu and the page context menu, rebuilt on every language
//! change with labels from the browser's translations. Standard items keep
//! their AppKit selectors so system behaviour (Services, tabs, Edit) is intact.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{sel, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{NSEventModifierFlags, NSMenu, NSMenuItem, NSUserInterfaceItemIdentification};
use objc2_foundation::NSString;

use llmouser_browser::automation::MenuItemInfo;
use llmouser_browser::i18n::{fill, Messages};
use llmouser_browser::{Language, TabId, APP_NAME};

use super::app::{self, state};
use super::util::ns;

fn item(
    mtm: MainThreadMarker,
    id: &str,
    title: &str,
    action: Option<Sel>,
    key: &str,
    mask: Option<NSEventModifierFlags>,
    target: Option<&AnyObject>,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &ns(title),
            action,
            &ns(key),
        )
    };
    if let Some(mask) = mask {
        item.setKeyEquivalentModifierMask(mask);
    }
    if let Some(target) = target {
        unsafe { item.setTarget(Some(target)) };
    }
    item.setIdentifier(Some(&ns(id)));
    item
}

fn separator(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    NSMenuItem::separatorItem(mtm)
}

fn submenu(
    mtm: MainThreadMarker,
    id: &str,
    title: &str,
    items: Vec<Retained<NSMenuItem>>,
) -> (Retained<NSMenuItem>, Retained<NSMenu>) {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns(title));
    for i in items {
        menu.addItem(&i);
    }
    let holder = item(mtm, id, title, None, "", None, None);
    holder.setSubmenu(Some(&menu));
    (holder, menu)
}

const CMD: NSEventModifierFlags = NSEventModifierFlags::Command;

fn cmd_shift() -> NSEventModifierFlags {
    NSEventModifierFlags::Command | NSEventModifierFlags::Shift
}

fn cmd_option() -> NSEventModifierFlags {
    NSEventModifierFlags::Command | NSEventModifierFlags::Option
}

fn cmd_ctrl() -> NSEventModifierFlags {
    NSEventModifierFlags::Command | NSEventModifierFlags::Control
}

/// Build and install the main menu for `language`.
pub fn install(language: Language) {
    let st = state();
    let mtm = st.mtm;
    let m: &Messages = language.messages();
    let d: &AnyObject = st.delegate.as_target();
    let app_name = APP_NAME;

    let (services_item, services_menu) = submenu(mtm, "services", m.menu_services, vec![]);
    let (app_item, _) = submenu(
        mtm,
        "app",
        app_name,
        vec![
            item(
                mtm,
                "about",
                &fill(m.menu_about, &[("app", app_name)]),
                Some(sel!(showAbout:)),
                "",
                None,
                Some(d),
            ),
            separator(mtm),
            item(
                mtm,
                "settings",
                m.menu_settings,
                Some(sel!(openSettings:)),
                ",",
                Some(CMD),
                Some(d),
            ),
            separator(mtm),
            services_item,
            separator(mtm),
            item(
                mtm,
                "hide",
                &fill(m.menu_hide, &[("app", app_name)]),
                Some(sel!(hide:)),
                "h",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "hide-others",
                m.menu_hide_others,
                Some(sel!(hideOtherApplications:)),
                "h",
                Some(cmd_option()),
                None,
            ),
            item(
                mtm,
                "show-all",
                m.menu_show_all,
                Some(sel!(unhideAllApplications:)),
                "",
                None,
                None,
            ),
            separator(mtm),
            item(
                mtm,
                "quit",
                &fill(m.menu_quit, &[("app", app_name)]),
                Some(sel!(terminate:)),
                "q",
                Some(CMD),
                None,
            ),
        ],
    );

    let (file_item, _) = submenu(
        mtm,
        "file",
        m.menu_file,
        vec![
            item(
                mtm,
                "new-tab",
                m.menu_new_tab,
                Some(sel!(newTab:)),
                "t",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "new-window",
                m.menu_new_window,
                Some(sel!(newWindow:)),
                "n",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "reopen-closed-tab",
                m.menu_reopen_closed_tab,
                Some(sel!(reopenClosedTab:)),
                "t",
                Some(cmd_shift()),
                Some(d),
            ),
            separator(mtm),
            item(
                mtm,
                "close-tab",
                m.menu_close_tab,
                Some(sel!(closeTab:)),
                "w",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "close-window",
                m.menu_close_window,
                Some(sel!(closeWindow:)),
                "w",
                Some(cmd_shift()),
                Some(d),
            ),
            separator(mtm),
            item(
                mtm,
                "save-pdf",
                m.menu_save_pdf,
                Some(sel!(savePdf:)),
                "s",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "save-html",
                m.menu_save_html,
                Some(sel!(saveHtml:)),
                "s",
                Some(cmd_shift()),
                Some(d),
            ),
        ],
    );

    let (edit_item, _) = submenu(
        mtm,
        "edit",
        m.menu_edit,
        vec![
            item(
                mtm,
                "undo",
                m.menu_undo,
                Some(sel!(undo:)),
                "z",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "redo",
                m.menu_redo,
                Some(sel!(redo:)),
                "z",
                Some(cmd_shift()),
                None,
            ),
            separator(mtm),
            item(
                mtm,
                "cut",
                m.menu_cut,
                Some(sel!(cut:)),
                "x",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "copy",
                m.menu_copy,
                Some(sel!(copy:)),
                "c",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "paste",
                m.menu_paste,
                Some(sel!(paste:)),
                "v",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "paste-match-style",
                m.menu_paste_match_style,
                Some(sel!(pasteAsPlainText:)),
                "v",
                Some(cmd_option() | NSEventModifierFlags::Shift),
                None,
            ),
            item(
                mtm,
                "delete",
                m.menu_delete,
                Some(sel!(delete:)),
                "",
                None,
                None,
            ),
            item(
                mtm,
                "select-all",
                m.menu_select_all,
                Some(sel!(selectAll:)),
                "a",
                Some(CMD),
                None,
            ),
        ],
    );

    let (view_item, _) = submenu(
        mtm,
        "view",
        m.menu_view,
        vec![
            item(
                mtm,
                "open-location",
                m.menu_open_location,
                Some(sel!(focusAddress:)),
                "l",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "reload",
                m.menu_reload,
                Some(sel!(reloadPage:)),
                "r",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "stop",
                m.menu_stop,
                Some(sel!(stopLoading:)),
                ".",
                Some(CMD),
                Some(d),
            ),
            separator(mtm),
            item(
                mtm,
                "reset-zoom",
                m.menu_reset_zoom,
                Some(sel!(resetZoom:)),
                "0",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "zoom-in",
                m.menu_zoom_in,
                Some(sel!(zoomIn:)),
                "=",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "zoom-out",
                m.menu_zoom_out,
                Some(sel!(zoomOut:)),
                "-",
                Some(CMD),
                Some(d),
            ),
            separator(mtm),
            item(
                mtm,
                "fullscreen",
                m.menu_toggle_fullscreen,
                Some(sel!(toggleFullScreen:)),
                "f",
                Some(cmd_ctrl()),
                None,
            ),
        ],
    );

    let (history_item, _) = submenu(
        mtm,
        "history",
        m.menu_history,
        vec![
            item(
                mtm,
                "back",
                m.menu_back,
                Some(sel!(goBack:)),
                "[",
                Some(CMD),
                Some(d),
            ),
            item(
                mtm,
                "forward",
                m.menu_forward,
                Some(sel!(goForward:)),
                "]",
                Some(CMD),
                Some(d),
            ),
        ],
    );

    let (window_item, window_menu) = submenu(
        mtm,
        "window",
        m.menu_window,
        vec![
            item(
                mtm,
                "minimize",
                m.menu_minimize,
                Some(sel!(performMiniaturize:)),
                "m",
                Some(CMD),
                None,
            ),
            item(
                mtm,
                "zoom-window",
                m.menu_zoom_window,
                Some(sel!(performZoom:)),
                "",
                None,
                None,
            ),
            separator(mtm),
            item(
                mtm,
                "previous-tab",
                m.menu_show_previous_tab,
                Some(sel!(selectPreviousTab:)),
                "\u{19}",
                Some(NSEventModifierFlags::Control | NSEventModifierFlags::Shift),
                None,
            ),
            item(
                mtm,
                "next-tab",
                m.menu_show_next_tab,
                Some(sel!(selectNextTab:)),
                "\u{9}",
                Some(NSEventModifierFlags::Control),
                None,
            ),
            separator(mtm),
            item(
                mtm,
                "front",
                m.menu_front,
                Some(sel!(arrangeInFront:)),
                "",
                None,
                None,
            ),
        ],
    );

    let (help_item, help_menu) = submenu(
        mtm,
        "help",
        m.menu_help,
        vec![item(
            mtm,
            "homepage",
            &fill(m.menu_homepage, &[("app", app_name)]),
            Some(sel!(openHomepage:)),
            "",
            None,
            Some(d),
        )],
    );

    let main = NSMenu::initWithTitle(NSMenu::alloc(mtm), &ns("Main"));
    for top in [
        app_item,
        file_item,
        edit_item,
        view_item,
        history_item,
        window_item,
        help_item,
    ] {
        main.addItem(&top);
    }
    let app = app::app(mtm);
    app.setMainMenu(Some(&main));
    app.setServicesMenu(Some(&services_menu));
    app.setWindowsMenu(Some(&window_menu));
    app.setHelpMenu(Some(&help_menu));
}

/// Enabled state of the delegate's own menu items.
pub fn validate(item: &NSMenuItem) -> bool {
    let id = item.identifier().map(|i| i.to_string()).unwrap_or_default();
    let st = state();
    let current = app::current_window();
    let session = st.session.borrow();
    let tab = current.as_ref().and_then(|w| session.browser.tab(w.tab));
    match id.as_str() {
        "back" => tab.map(|t| t.can_back()).unwrap_or(false),
        "forward" => tab.map(|t| t.can_forward()).unwrap_or(false),
        "stop" => tab.map(|t| t.is_loading()).unwrap_or(false),
        "reload" => tab
            .map(|t| !matches!(t.content, llmouser_browser::Content::Empty))
            .unwrap_or(false),
        "reopen-closed-tab" => session.browser.closed_count() > 0,
        "close-window" | "save-pdf" | "save-html" | "open-location" | "reset-zoom" | "zoom-in"
        | "zoom-out" => current.is_some(),
        _ => true,
    }
}

/// Fill the page's context menu: edit actions plus the save commands.
pub fn populate_context_menu(menu: &NSMenu, _tab: TabId) {
    let st = state();
    let mtm = st.mtm;
    let m = st.session.borrow().messages();
    let d: &AnyObject = st.delegate.as_target();
    menu.removeAllItems();
    menu.setAutoenablesItems(true);
    for i in [
        item(mtm, "cut", m.menu_cut, Some(sel!(cut:)), "", None, None),
        item(mtm, "copy", m.menu_copy, Some(sel!(copy:)), "", None, None),
        item(
            mtm,
            "paste",
            m.menu_paste,
            Some(sel!(paste:)),
            "",
            None,
            None,
        ),
        separator(mtm),
        item(
            mtm,
            "save-pdf",
            m.menu_save_pdf,
            Some(sel!(savePdf:)),
            "",
            None,
            Some(d),
        ),
        item(
            mtm,
            "save-html",
            m.menu_save_html,
            Some(sel!(saveHtml:)),
            "",
            None,
            Some(d),
        ),
    ] {
        menu.addItem(&i);
    }
}

/// The menu as data, for the automation channel.
pub fn describe(menu: &NSMenu) -> Vec<MenuItemInfo> {
    menu.update();
    menu.itemArray()
        .iter()
        .filter(|i| !i.isSeparatorItem())
        .map(|i| MenuItemInfo {
            id: i.identifier().map(|s| s.to_string()).unwrap_or_default(),
            label: i.title().to_string(),
            enabled: i.isEnabled(),
            accelerator: accelerator(&i.keyEquivalent(), i.keyEquivalentModifierMask()),
            children: i.submenu().map(|s| describe(&s)).unwrap_or_default(),
        })
        .collect()
}

fn accelerator(key: &NSString, mask: NSEventModifierFlags) -> String {
    let key = key.to_string();
    if key.is_empty() {
        return String::new();
    }
    let mut parts = Vec::new();
    if mask.contains(NSEventModifierFlags::Control) {
        parts.push("ctrl");
    }
    if mask.contains(NSEventModifierFlags::Option) {
        parts.push("alt");
    }
    if mask.contains(NSEventModifierFlags::Shift) {
        parts.push("shift");
    }
    if mask.contains(NSEventModifierFlags::Command) {
        parts.push("cmd");
    }
    let key = match key.as_str() {
        "\u{9}" => "tab".to_string(),
        "\u{19}" => "tab".to_string(),
        other => other.to_string(),
    };
    parts.push(&key);
    parts.join("+")
}

/// Find an item anywhere in the menu tree by identifier.
pub fn find_item(menu: &NSMenu, id: &str) -> Option<(Retained<NSMenu>, Retained<NSMenuItem>)> {
    for i in menu.itemArray().iter() {
        if i.identifier().map(|s| s.to_string()).as_deref() == Some(id) {
            return Some((menu.retain(), i));
        }
        if let Some(sub) = i.submenu() {
            if let Some(found) = find_item(&sub, id) {
                return Some(found);
            }
        }
    }
    None
}
