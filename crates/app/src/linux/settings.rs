//! The Settings dialog: an `AdwDialog` with two pages (LLM, UX) of preference rows.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use llmouser_browser::automation::SettingsInfo;
use llmouser_browser::settings::Provider;
use llmouser_browser::Language;

use super::app::{self, state};
use crate::session::SettingsForm;

pub struct SettingsDialog {
    pub dialog: adw::Dialog,
    stack: adw::ViewStack,
    pub provider: adw::ComboRow,
    pub endpoint: adw::EntryRow,
    pub model: adw::EntryRow,
    pub api_key: adw::PasswordEntryRow,
    pub max_tokens: adw::EntryRow,
    pub universe: gtk::TextView,
    universe_group: adw::PreferencesGroup,
    pub language: adw::ComboRow,
    pub search_url: adw::EntryRow,
    pub save: gtk::Button,
    pub close: gtk::Button,
    pub validation: gtk::Label,
    has_api_key: Cell<bool>,
    open: Cell<bool>,
    quiet: Cell<bool>,
}

impl SettingsDialog {
    pub fn new() -> Rc<SettingsDialog> {
        let m = state().session.borrow().messages();

        let provider = adw::ComboRow::new();
        let providers: Vec<&str> = Provider::ALL.iter().map(|p| p.label()).collect();
        provider.set_model(Some(&gtk::StringList::new(&providers)));
        let endpoint = adw::EntryRow::new();
        let model = adw::EntryRow::new();
        let api_key = adw::PasswordEntryRow::new();
        let max_tokens = adw::EntryRow::new();
        max_tokens.set_input_purpose(gtk::InputPurpose::Digits);

        let llm_group = adw::PreferencesGroup::new();
        for row in [
            provider.upcast_ref::<gtk::Widget>(),
            endpoint.upcast_ref(),
            model.upcast_ref(),
            api_key.upcast_ref(),
            max_tokens.upcast_ref(),
        ] {
            llm_group.add(row);
        }
        let universe = gtk::TextView::new();
        universe.set_wrap_mode(gtk::WrapMode::WordChar);
        universe.set_top_margin(6);
        universe.set_bottom_margin(6);
        universe.set_left_margin(8);
        universe.set_right_margin(8);
        let universe_scroll = gtk::ScrolledWindow::builder()
            .child(&universe)
            .min_content_height(90)
            .max_content_height(160)
            .propagate_natural_height(true)
            .has_frame(true)
            .build();
        universe_scroll.add_css_class("card");
        let universe_group = adw::PreferencesGroup::new();
        universe_group.add(&universe_scroll);
        let llm_page = adw::PreferencesPage::new();
        llm_page.add(&llm_group);
        llm_page.add(&universe_group);

        let language = adw::ComboRow::new();
        let languages: Vec<&str> = Language::ALL
            .iter()
            .map(|l| l.messages().language_name)
            .collect();
        language.set_model(Some(&gtk::StringList::new(&languages)));
        let search_url = adw::EntryRow::new();
        let ux_group = adw::PreferencesGroup::new();
        ux_group.add(&language);
        ux_group.add(&search_url);
        let ux_page = adw::PreferencesPage::new();
        ux_page.add(&ux_group);

        let stack = adw::ViewStack::new();
        stack.add_titled(&llm_page, Some("llm"), m.settings_tab_llm);
        stack.add_titled(&ux_page, Some("ux"), m.settings_tab_ux);
        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);

        let validation = gtk::Label::new(None);
        validation.add_css_class("error");
        validation.set_wrap(true);
        validation.set_margin_start(12);
        validation.set_margin_end(12);
        validation.set_margin_bottom(6);

        let close = gtk::Button::with_label(m.close);
        let save = gtk::Button::with_label(m.save);
        save.add_css_class("suggested-action");
        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);
        header.set_title_widget(Some(&switcher));
        header.pack_start(&close);
        header.pack_end(&save);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&stack);
        content.append(&validation);
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));

        let dialog = adw::Dialog::new();
        dialog.set_title(m.settings_title);
        dialog.set_content_width(600);
        dialog.set_content_height(560);
        dialog.set_child(Some(&toolbar_view));

        let this = Rc::new(SettingsDialog {
            dialog,
            stack,
            provider,
            endpoint,
            model,
            api_key,
            max_tokens,
            universe,
            universe_group,
            language,
            search_url,
            save,
            close,
            validation,
            has_api_key: Cell::new(false),
            open: Cell::new(false),
            quiet: Cell::new(false),
        });
        this.wire();
        this.retranslate();
        this
    }

    fn wire(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.provider.connect_selected_notify(move |_| {
            if let Some(s) = me.upgrade() {
                if !s.quiet.get() {
                    s.provider_changed();
                }
            }
        });
        let me = Rc::downgrade(self);
        self.language.connect_selected_notify(move |_| {
            if let Some(s) = me.upgrade() {
                if !s.quiet.get() {
                    app::set_language(s.picked_language());
                }
            }
        });
        let me = Rc::downgrade(self);
        self.save.connect_clicked(move |_| {
            if let Some(s) = me.upgrade() {
                s.save_clicked();
            }
        });
        let me = Rc::downgrade(self);
        self.close.connect_clicked(move |_| {
            if let Some(s) = me.upgrade() {
                s.close();
            }
        });
        let me = Rc::downgrade(self);
        self.dialog.connect_closed(move |_| {
            if let Some(s) = me.upgrade() {
                s.open.set(false);
            }
        });
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

    pub fn show(&self) {
        let (form, has_key) = {
            let st = state();
            let s = st.session.borrow();
            (s.settings_form(), s.public_settings().has_api_key)
        };
        self.has_api_key.set(has_key);
        self.apply(&form);
        self.stack.set_visible_child_name("llm");
        self.validation.set_text("");
        self.retranslate();
        if !self.open.get() {
            self.open.set(true);
            self.dialog.present(Some(&app::main_window().window));
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn close(&self) {
        self.open.set(false);
        self.dialog.close();
    }

    pub fn apply(&self, form: &SettingsForm) {
        self.quiet.set(true);
        self.provider.set_selected(
            Provider::ALL
                .iter()
                .position(|p| *p == form.provider)
                .unwrap_or(0) as u32,
        );
        self.endpoint.set_text(&form.endpoint);
        self.model.set_text(&form.model);
        self.api_key.set_text("");
        self.max_tokens.set_text(&form.max_tokens);
        self.universe.buffer().set_text(&form.universe);
        self.language.set_selected(
            Language::ALL
                .iter()
                .position(|l| *l == form.language)
                .unwrap_or(0) as u32,
        );
        self.search_url.set_text(&form.search_url);
        self.quiet.set(false);
    }

    pub fn form(&self) -> SettingsForm {
        SettingsForm {
            provider: self.picked_provider(),
            endpoint: self.endpoint.text().to_string(),
            model: self.model.text().to_string(),
            api_key: self.api_key.text().to_string(),
            max_tokens: self.max_tokens.text().to_string(),
            universe: self.universe_text(),
            language: self.picked_language(),
            search_url: self.search_url.text().to_string(),
        }
    }

    fn universe_text(&self) -> String {
        let buffer = self.universe.buffer();
        buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string()
    }

    pub fn picked_provider(&self) -> Provider {
        Provider::ALL
            .get(self.provider.selected() as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn picked_language(&self) -> Language {
        Language::ALL
            .get(self.language.selected() as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn provider_changed(&self) {
        let p = self.picked_provider();
        self.endpoint.set_text(p.default_endpoint());
        self.model.set_text(p.default_model());
    }

    pub fn show_validation(&self, text: &str) {
        self.validation.set_text(text);
    }

    pub fn select_tab(&self, tab: &str) {
        self.stack
            .set_visible_child_name(if tab == "ux" { "ux" } else { "llm" });
    }

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        self.dialog.set_title(m.settings_title);
        if let Some(page) = self.stack.child_by_name("llm").map(|c| self.stack.page(&c)) {
            page.set_title(Some(m.settings_tab_llm));
        }
        if let Some(page) = self.stack.child_by_name("ux").map(|c| self.stack.page(&c)) {
            page.set_title(Some(m.settings_tab_ux));
        }
        self.provider.set_title(m.provider);
        self.endpoint.set_title(m.endpoint);
        self.model.set_title(m.model);
        self.api_key.set_title(if self.has_api_key.get() {
            m.api_key_saved
        } else {
            m.api_key_enter
        });
        self.max_tokens.set_title(m.max_tokens);
        self.universe_group.set_title(m.universe_rules);
        self.universe_group
            .set_description(Some(m.universe_placeholder));
        self.language.set_title(m.language);
        self.search_url.set_title(m.search_url);
        self.search_url.set_tooltip_text(Some(m.search_url_hint));
        self.save.set_label(m.save);
        self.close.set_label(m.close);
    }

    pub fn info(&self) -> SettingsInfo {
        let m = state().session.borrow().messages();
        SettingsInfo {
            open: self.is_open(),
            title: self.dialog.title().to_string(),
            tab: self
                .stack
                .visible_child_name()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            tab_labels: ["llm", "ux"]
                .iter()
                .filter_map(|n| self.stack.child_by_name(n).map(|c| self.stack.page(&c)))
                .map(|p| p.title().map(|t| t.to_string()).unwrap_or_default())
                .collect(),
            labels: vec![
                self.provider.title().to_string(),
                self.endpoint.title().to_string(),
                self.model.title().to_string(),
                m.api_key.to_string(),
                self.max_tokens.title().to_string(),
                self.universe_group.title().to_string(),
                self.language.title().to_string(),
                self.search_url.title().to_string(),
            ],
            provider: self.picked_provider().id().into(),
            provider_options: Provider::ALL
                .iter()
                .map(|p| p.label().to_string())
                .collect(),
            endpoint: self.endpoint.text().to_string(),
            model: self.model.text().to_string(),
            api_key: self.api_key.text().to_string(),
            api_key_placeholder: self.api_key.title().to_string(),
            max_tokens: self.max_tokens.text().to_string(),
            universe: self.universe_text(),
            universe_placeholder: m.universe_placeholder.to_string(),
            language: self.picked_language().id().into(),
            language_options: Language::ALL
                .iter()
                .map(|l| l.messages().language_name.to_string())
                .collect(),
            search_url: self.search_url.text().to_string(),
            save_label: self.save.label().map(|s| s.to_string()).unwrap_or_default(),
            close_label: self
                .close
                .label()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            validation: self.validation.text().to_string(),
        }
    }

    pub fn set_field(&self, field: &str, value: &str) -> Result<(), String> {
        match field {
            "provider" => {
                let p =
                    Provider::from_id(value).ok_or_else(|| format!("unknown provider {value}"))?;
                self.provider
                    .set_selected(Provider::ALL.iter().position(|x| *x == p).unwrap_or(0) as u32);
            }
            "language" => {
                let l =
                    Language::from_id(value).ok_or_else(|| format!("unknown language {value}"))?;
                self.language
                    .set_selected(Language::ALL.iter().position(|x| *x == l).unwrap_or(0) as u32);
            }
            "endpoint" => self.endpoint.set_text(value),
            "model" => self.model.set_text(value),
            "api_key" => self.api_key.set_text(value),
            "max_tokens" => self.max_tokens.set_text(value),
            "universe" => self.universe.buffer().set_text(value),
            "search_url" => self.search_url.set_text(value),
            other => return Err(format!("unknown settings field {other}")),
        }
        Ok(())
    }
}
