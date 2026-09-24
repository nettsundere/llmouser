//! The Settings window: a native form with two tabs (LLM, UX).

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSButton, NSColor, NSFont, NSGridCellPlacement, NSGridView,
    NSLayoutConstraint, NSPopUpButton, NSScrollView, NSSecureTextField, NSStackView, NSTabView,
    NSTabViewItem, NSTextField, NSTextView, NSUserInterfaceLayoutOrientation, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSString};

use llmouser_browser::automation::SettingsInfo;
use llmouser_browser::i18n::Messages;
use llmouser_browser::settings::Provider;
use llmouser_browser::Language;

use super::app::state;
use super::util::{ns, rect, size};
use crate::session::SettingsForm;

type LabelText = fn(&Messages) -> &'static str;

pub struct SettingsWindow {
    pub window: Retained<NSWindow>,
    tabs: Retained<NSTabView>,
    llm_item: Retained<NSTabViewItem>,
    ux_item: Retained<NSTabViewItem>,
    pub provider: Retained<NSPopUpButton>,
    pub endpoint: Retained<NSTextField>,
    pub model: Retained<NSTextField>,
    pub api_key: Retained<NSSecureTextField>,
    pub max_tokens: Retained<NSTextField>,
    pub universe: Retained<NSTextView>,
    pub language: Retained<NSPopUpButton>,
    pub search_url: Retained<NSTextField>,
    pub search_hint: Retained<NSTextField>,
    pub save: Retained<NSButton>,
    pub close: Retained<NSButton>,
    pub validation: Retained<NSTextField>,
    labels: RefCell<Vec<(Retained<NSTextField>, LabelText)>>,
    has_api_key: RefCell<bool>,
}

const FIELD_WIDTH: f64 = 320.0;

fn field(mtm: MainThreadMarker) -> Retained<NSTextField> {
    let f = NSTextField::textFieldWithString(&ns(""), mtm);
    f.widthAnchor()
        .constraintEqualToConstant(FIELD_WIDTH)
        .setActive(true);
    f
}

fn label(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    NSTextField::labelWithString(&ns(text), mtm)
}

fn popup(
    mtm: MainThreadMarker,
    titles: &[&str],
    target: &AnyObject,
    action: objc2::runtime::Sel,
) -> Retained<NSPopUpButton> {
    let p = NSPopUpButton::initWithFrame_pullsDown(
        NSPopUpButton::alloc(mtm),
        rect(0.0, 0.0, FIELD_WIDTH, 26.0),
        false,
    );
    let titles: Vec<Retained<NSString>> = titles.iter().map(|t| ns(t)).collect();
    p.addItemsWithTitles(&NSArray::from_retained_slice(&titles));
    p.widthAnchor()
        .constraintEqualToConstant(FIELD_WIDTH)
        .setActive(true);
    unsafe {
        p.setTarget(Some(target));
        p.setAction(Some(action));
    }
    p
}

fn grid(mtm: MainThreadMarker, rows: &[(&NSTextField, &NSView)]) -> Retained<NSGridView> {
    let row_arrays: Vec<Retained<NSArray<NSView>>> = rows
        .iter()
        .map(|(l, f)| {
            let l: &NSView = l;
            NSArray::from_slice(&[l, f])
        })
        .collect();
    let g = NSGridView::gridViewWithViews(&NSArray::from_retained_slice(&row_arrays), mtm);
    g.setRowSpacing(10.0);
    g.setColumnSpacing(12.0);
    g.setYPlacement(NSGridCellPlacement::Center);
    g.columnAtIndex(0)
        .setXPlacement(NSGridCellPlacement::Trailing);
    g
}

impl SettingsWindow {
    pub fn new(mtm: MainThreadMarker) -> Rc<SettingsWindow> {
        let st = state();
        let m = st.session.borrow().messages();
        let d: &AnyObject = st.delegate.as_target();

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 560.0, 470.0),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&ns(m.settings_title));

        // LLM tab.
        let provider_titles: Vec<&str> = Provider::ALL.iter().map(|p| p.label()).collect();
        let provider = popup(mtm, &provider_titles, d, sel!(providerChanged:));
        let endpoint = field(mtm);
        let model = field(mtm);
        let api_key = NSSecureTextField::new(mtm);
        api_key
            .widthAnchor()
            .constraintEqualToConstant(FIELD_WIDTH)
            .setActive(true);
        let max_tokens = field(mtm);
        let universe_scroll: Retained<NSScrollView> = NSTextView::scrollableTextView(mtm);
        universe_scroll
            .widthAnchor()
            .constraintEqualToConstant(FIELD_WIDTH)
            .setActive(true);
        universe_scroll
            .heightAnchor()
            .constraintEqualToConstant(72.0)
            .setActive(true);
        let universe: Retained<NSTextView> =
            unsafe { Retained::cast_unchecked(universe_scroll.documentView().expect("text view")) };
        universe.setFont(Some(&NSFont::systemFontOfSize(NSFont::systemFontSize())));
        universe.setRichText(false);
        universe.setAutomaticQuoteSubstitutionEnabled(false);
        universe.setAutomaticDashSubstitutionEnabled(false);

        let l_provider = label(mtm, m.provider);
        let l_endpoint = label(mtm, m.endpoint);
        let l_model = label(mtm, m.model);
        let l_api_key = label(mtm, m.api_key);
        let l_max_tokens = label(mtm, m.max_tokens);
        let l_universe = label(mtm, m.universe_rules);
        let llm_grid = grid(
            mtm,
            &[
                (&l_provider, &provider),
                (&l_endpoint, &endpoint),
                (&l_model, &model),
                (&l_api_key, &api_key),
                (&l_max_tokens, &max_tokens),
                (&l_universe, &universe_scroll),
            ],
        );

        // UX tab.
        let language_titles: Vec<&str> = Language::ALL
            .iter()
            .map(|l| l.messages().language_name)
            .collect();
        let language = popup(mtm, &language_titles, d, sel!(languageChanged:));
        let search_url = field(mtm);
        let search_hint = NSTextField::wrappingLabelWithString(&ns(m.search_url_hint), mtm);
        search_hint.setFont(Some(&NSFont::systemFontOfSize(
            NSFont::smallSystemFontSize(),
        )));
        search_hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
        search_hint.setPreferredMaxLayoutWidth(FIELD_WIDTH);
        search_hint
            .widthAnchor()
            .constraintEqualToConstant(FIELD_WIDTH)
            .setActive(true);
        let l_language = label(mtm, m.language);
        let l_search = label(mtm, m.search_url);
        let l_hint = label(mtm, "");
        let ux_grid = grid(
            mtm,
            &[
                (&l_language, &language),
                (&l_search, &search_url),
                (&l_hint, &search_hint),
            ],
        );

        let tabs = NSTabView::initWithFrame(NSTabView::alloc(mtm), rect(0.0, 0.0, 520.0, 330.0));
        let llm_item = NSTabViewItem::new();
        llm_item.setLabel(&ns(m.settings_tab_llm));
        llm_item.setView(Some(&pad(mtm, &llm_grid)));
        let ux_item = NSTabViewItem::new();
        ux_item.setLabel(&ns(m.settings_tab_ux));
        ux_item.setView(Some(&pad(mtm, &ux_grid)));
        tabs.addTabViewItem(&llm_item);
        tabs.addTabViewItem(&ux_item);
        tabs.heightAnchor()
            .constraintEqualToConstant(340.0)
            .setActive(true);

        let validation = NSTextField::wrappingLabelWithString(&ns(""), mtm);
        validation.setTextColor(Some(&NSColor::systemRedColor()));
        validation.setPreferredMaxLayoutWidth(500.0);

        let save = unsafe {
            NSButton::buttonWithTitle_target_action(
                &ns(m.save),
                Some(d),
                Some(sel!(saveSettings:)),
                mtm,
            )
        };
        save.setKeyEquivalent(&ns("\r"));
        let close = unsafe {
            NSButton::buttonWithTitle_target_action(
                &ns(m.close),
                Some(d),
                Some(sel!(closeSettings:)),
                mtm,
            )
        };
        close.setKeyEquivalent(&ns("\u{1b}"));
        let buttons = NSStackView::stackViewWithViews(
            &NSArray::from_slice(&[&*save as &NSView, &*close as &NSView]),
            mtm,
        );
        buttons.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
        buttons.setSpacing(8.0);

        let stack = NSStackView::stackViewWithViews(
            &NSArray::from_slice(&[
                &*tabs as &NSView,
                &*validation as &NSView,
                &*buttons as &NSView,
            ]),
            mtm,
        );
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
        stack.setSpacing(10.0);
        stack.setEdgeInsets(NSEdgeInsets {
            top: 16.0,
            left: 16.0,
            bottom: 16.0,
            right: 16.0,
        });
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        let content = window.contentView().expect("content view");
        content.addSubview(&stack);
        NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
            stack
                .topAnchor()
                .constraintEqualToAnchor(&content.topAnchor()),
            stack
                .leadingAnchor()
                .constraintEqualToAnchor(&content.leadingAnchor()),
            stack
                .trailingAnchor()
                .constraintEqualToAnchor(&content.trailingAnchor()),
            stack
                .bottomAnchor()
                .constraintEqualToAnchor(&content.bottomAnchor()),
        ]));

        let labels: Vec<(Retained<NSTextField>, LabelText)> = vec![
            (l_provider, |m| m.provider),
            (l_endpoint, |m| m.endpoint),
            (l_model, |m| m.model),
            (l_api_key, |m| m.api_key),
            (l_max_tokens, |m| m.max_tokens),
            (l_universe, |m| m.universe_rules),
            (l_language, |m| m.language),
            (l_search, |m| m.search_url),
        ];

        Rc::new(SettingsWindow {
            window,
            tabs,
            llm_item,
            ux_item,
            provider,
            endpoint,
            model,
            api_key,
            max_tokens,
            universe,
            language,
            search_url,
            search_hint,
            save,
            close,
            validation,
            labels: RefCell::new(labels),
            has_api_key: RefCell::new(false),
        })
    }

    /// Fill from the stored settings, reset to the first tab, and bring to front.
    pub fn show(&self) {
        let (form, has_key) = {
            let st = state();
            let s = st.session.borrow();
            (s.settings_form(), s.public_settings().has_api_key)
        };
        *self.has_api_key.borrow_mut() = has_key;
        self.apply(&form);
        self.tabs.selectTabViewItemAtIndex(0);
        self.validation.setStringValue(&ns(""));
        if !self.window.isVisible() {
            self.window.center();
        }
        self.window.makeKeyAndOrderFront(None);
        self.retranslate();
    }

    pub fn close(&self) {
        self.window.close();
    }

    pub fn is_open(&self) -> bool {
        self.window.isVisible()
    }

    pub fn apply(&self, form: &SettingsForm) {
        self.provider.selectItemAtIndex(
            Provider::ALL
                .iter()
                .position(|p| *p == form.provider)
                .unwrap_or(0) as isize,
        );
        self.endpoint.setStringValue(&ns(&form.endpoint));
        self.model.setStringValue(&ns(&form.model));
        self.api_key.setStringValue(&ns(""));
        self.max_tokens.setStringValue(&ns(&form.max_tokens));
        self.universe.setString(&ns(&form.universe));
        self.language.selectItemAtIndex(
            Language::ALL
                .iter()
                .position(|l| *l == form.language)
                .unwrap_or(0) as isize,
        );
        self.search_url.setStringValue(&ns(&form.search_url));
    }

    pub fn form(&self) -> SettingsForm {
        SettingsForm {
            provider: self.picked_provider(),
            endpoint: self.endpoint.stringValue().to_string(),
            model: self.model.stringValue().to_string(),
            api_key: self.api_key.stringValue().to_string(),
            max_tokens: self.max_tokens.stringValue().to_string(),
            universe: self.universe.string().to_string(),
            language: self.picked_language(),
            search_url: self.search_url.stringValue().to_string(),
        }
    }

    pub fn picked_provider(&self) -> Provider {
        Provider::ALL
            .get(self.provider.indexOfSelectedItem().max(0) as usize)
            .copied()
            .unwrap_or_default()
    }

    pub fn picked_language(&self) -> Language {
        Language::ALL
            .get(self.language.indexOfSelectedItem().max(0) as usize)
            .copied()
            .unwrap_or_default()
    }

    /// When the provider changes, prefill sensible endpoint/model defaults.
    pub fn provider_changed(&self) {
        let p = self.picked_provider();
        self.endpoint.setStringValue(&ns(p.default_endpoint()));
        self.model.setStringValue(&ns(p.default_model()));
    }

    pub fn show_validation(&self, text: &str) {
        self.validation.setStringValue(&ns(text));
    }

    pub fn select_tab(&self, tab: &str) {
        self.tabs
            .selectTabViewItemAtIndex(if tab == "ux" { 1 } else { 0 });
    }

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        self.window.setTitle(&ns(m.settings_title));
        self.llm_item.setLabel(&ns(m.settings_tab_llm));
        self.ux_item.setLabel(&ns(m.settings_tab_ux));
        for (label, text) in self.labels.borrow().iter() {
            label.setStringValue(&ns(text(m)));
        }
        self.search_hint.setStringValue(&ns(m.search_url_hint));
        self.save.setTitle(&ns(m.save));
        self.close.setTitle(&ns(m.close));
        let placeholder = if *self.has_api_key.borrow() {
            m.api_key_saved
        } else {
            m.api_key_enter
        };
        self.api_key.setPlaceholderString(Some(&ns(placeholder)));
        self.universe.setString(&self.universe.string());
    }

    pub fn info(&self) -> SettingsInfo {
        let m = state().session.borrow().messages();
        let selected = self.tabs.selectedTabViewItem();
        let tab = match selected {
            Some(item) if std::ptr::eq(&*item, &*self.ux_item) => "ux",
            _ => "llm",
        };
        SettingsInfo {
            open: self.is_open(),
            title: self.window.title().to_string(),
            tab: tab.into(),
            tab_labels: vec![
                self.llm_item.label().to_string(),
                self.ux_item.label().to_string(),
            ],
            labels: self
                .labels
                .borrow()
                .iter()
                .map(|(l, _)| l.stringValue().to_string())
                .collect(),
            provider: self.picked_provider().id().into(),
            provider_options: self
                .provider
                .itemTitles()
                .iter()
                .map(|t| t.to_string())
                .collect(),
            endpoint: self.endpoint.stringValue().to_string(),
            model: self.model.stringValue().to_string(),
            api_key: self.api_key.stringValue().to_string(),
            api_key_placeholder: self
                .api_key
                .placeholderString()
                .map(|s| s.to_string())
                .unwrap_or_default(),
            max_tokens: self.max_tokens.stringValue().to_string(),
            universe: self.universe.string().to_string(),
            universe_placeholder: m.universe_placeholder.to_string(),
            language: self.picked_language().id().into(),
            language_options: self
                .language
                .itemTitles()
                .iter()
                .map(|t| t.to_string())
                .collect(),
            search_url: self.search_url.stringValue().to_string(),
            save_label: self.save.title().to_string(),
            close_label: self.close.title().to_string(),
            validation: self.validation.stringValue().to_string(),
        }
    }

    /// Set a field the way a user would edit the control.
    pub fn set_field(&self, field: &str, value: &str) -> Result<(), String> {
        let st = state();
        let app = super::app::app(st.mtm);
        let d: &AnyObject = st.delegate.as_target();
        match field {
            "provider" => {
                let index = Provider::from_id(value)
                    .and_then(|p| Provider::ALL.iter().position(|x| *x == p));
                let index = index.ok_or_else(|| format!("unknown provider {value}"))?;
                self.provider.selectItemAtIndex(index as isize);
                unsafe {
                    app.sendAction_to_from(sel!(providerChanged:), Some(d), Some(&self.provider))
                };
            }
            "language" => {
                let index = Language::from_id(value)
                    .and_then(|l| Language::ALL.iter().position(|x| *x == l));
                let index = index.ok_or_else(|| format!("unknown language {value}"))?;
                self.language.selectItemAtIndex(index as isize);
                unsafe {
                    app.sendAction_to_from(sel!(languageChanged:), Some(d), Some(&self.language))
                };
            }
            "endpoint" => self.endpoint.setStringValue(&ns(value)),
            "model" => self.model.setStringValue(&ns(value)),
            "api_key" => self.api_key.setStringValue(&ns(value)),
            "max_tokens" => self.max_tokens.setStringValue(&ns(value)),
            "universe" => self.universe.setString(&ns(value)),
            "search_url" => self.search_url.setStringValue(&ns(value)),
            other => return Err(format!("unknown settings field {other}")),
        }
        Ok(())
    }
}

/// Wrap a grid in a view with margins so tab content is not glued to the edges.
fn pad(mtm: MainThreadMarker, grid: &NSGridView) -> Retained<NSView> {
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, 500.0, 300.0));
    grid.setTranslatesAutoresizingMaskIntoConstraints(false);
    view.addSubview(grid);
    NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
        grid.topAnchor()
            .constraintEqualToAnchor_constant(&view.topAnchor(), 14.0),
        grid.leadingAnchor()
            .constraintEqualToAnchor_constant(&view.leadingAnchor(), 14.0),
    ]));
    let _ = size(0.0, 0.0);
    view
}
