//! The About dialog: centered icon, name, version, copyright.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;

use llmouser_browser::automation::AboutInfo;
use llmouser_browser::i18n::fill;
use llmouser_browser::{APP_COPYRIGHT, APP_NAME, APP_VERSION};

use super::app::{self, state, ICON_PNG};

pub struct AboutDialog {
    pub dialog: adw::Dialog,
    content: gtk::Box,
    icon: gtk::Image,
    name: gtk::Label,
    version: gtk::Label,
    copyright: gtk::Label,
    open: Cell<bool>,
    icon_loaded: bool,
}

impl AboutDialog {
    pub fn new() -> Rc<AboutDialog> {
        let texture = gdk::Texture::from_bytes(&glib::Bytes::from_static(ICON_PNG)).ok();
        let icon = gtk::Image::new();
        icon.set_pixel_size(128);
        if let Some(t) = &texture {
            icon.set_paintable(Some(t));
        }
        let name = gtk::Label::new(Some(APP_NAME));
        name.add_css_class("title-2");
        let version = gtk::Label::new(None);
        version.add_css_class("dim-label");
        let copyright = gtk::Label::new(Some(APP_COPYRIGHT));
        copyright.add_css_class("dim-label");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_valign(gtk::Align::Center);
        content.set_halign(gtk::Align::Fill);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        for w in [
            icon.upcast_ref::<gtk::Widget>(),
            name.upcast_ref(),
            version.upcast_ref(),
            copyright.upcast_ref(),
        ] {
            w.set_halign(gtk::Align::Center);
            content.append(w);
        }

        let header = adw::HeaderBar::new();
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));
        let dialog = adw::Dialog::new();
        dialog.set_content_width(320);
        dialog.set_content_height(360);
        dialog.set_child(Some(&toolbar_view));

        let this = Rc::new(AboutDialog {
            dialog,
            content,
            icon,
            name,
            version,
            copyright,
            open: Cell::new(false),
            icon_loaded: texture.is_some(),
        });
        let me = Rc::downgrade(&this);
        this.dialog.connect_closed(move |_| {
            if let Some(a) = me.upgrade() {
                a.open.set(false);
            }
        });
        this.retranslate();
        this
    }

    pub fn show(&self) {
        self.retranslate();
        if !self.open.get() {
            self.open.set(true);
            self.dialog.present(Some(&app::main_window().window));
        }
    }

    pub fn close(&self) {
        self.open.set(false);
        self.dialog.close();
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        self.dialog
            .set_title(&fill(m.menu_about, &[("app", APP_NAME)]));
        self.version
            .set_text(&format!("{} {}", m.about_version, APP_VERSION));
    }

    pub fn info(&self) -> AboutInfo {
        let off_center = self
            .icon
            .compute_bounds(&self.content)
            .map(|b| {
                let icon_center = b.x() as f64 + b.width() as f64 / 2.0;
                (icon_center - self.content.width() as f64 / 2.0).abs()
            })
            .unwrap_or(0.0);
        AboutInfo {
            open: self.is_open(),
            title: self.dialog.title().to_string(),
            name: self.name.text().to_string(),
            version_line: self.version.text().to_string(),
            copyright: self.copyright.text().to_string(),
            icon_loaded: self.icon_loaded,
            icon_off_center: off_center,
        }
    }
}
