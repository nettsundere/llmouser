//! The browser model: tabs, per-tab history, navigation sequencing and status.
//!
//! Pure state, no I/O. Shells call the mutators in response to user intent and
//! re-render from the getters; the [`Engine`](crate::Engine) runs the
//! generations this model asks for and reports back through [`Browser::finish`].

use serde::Serialize;

use crate::i18n::{fill, Messages};
use crate::llm::GenerateError;
use crate::page;
use crate::prompt::SiteRequest;
use crate::url::{normalize_url, resolve_input};

pub type TabId = u64;
pub type RequestId = u64;

/// Max visited URLs sent to the LLM as session context.
pub const HISTORY_LIMIT: usize = 10;
/// Max back/forward entries kept per tab.
pub const MAX_ENTRIES: usize = 50;
/// Max closed tabs kept for "Reopen Closed Tab".
pub const MAX_CLOSED: usize = 10;

/// One visited page, with its generated HTML cached so back/forward restores
/// the exact page instead of asking the LLM to generate a different one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Absolute URL of the page.
    pub url: String,
    /// Text that was shown in the address bar for this page.
    pub address: String,
    pub title: String,
    /// SVG favicon as a data URI, extracted from the generated page.
    pub icon: Option<String>,
    /// Raw generated HTML.
    pub html: String,
}

/// What the viewport of a tab shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// Fresh tab: the start page.
    Empty,
    /// A generated page (raw HTML; shells prepare it for display).
    Page { url: String, html: String },
    /// A failed generation.
    Error {
        url: String,
        message: String,
        offer_settings: bool,
    },
}

/// The status line, kept as data so a language switch retranslates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Status {
    Ready,
    Loading { url: String },
    Loaded { url: String },
    Failed { url: String, error: String },
    Stopped { url: String },
    SavingPdf,
    SavedPdf { path: String },
    PdfCanceled,
    FailedPdf { error: String },
    SavedHtml { path: String },
    HtmlCanceled,
    FailedHtml { error: String },
    NothingToSave,
}

impl Status {
    pub fn text(&self, m: &Messages) -> String {
        match self {
            Status::Ready => m.ready.to_string(),
            Status::Loading { url } => fill(m.loading, &[("url", url)]),
            Status::Loaded { url } => fill(m.loaded, &[("url", url)]),
            Status::Failed { url, error } => {
                fill(m.failed_to_load, &[("url", url), ("error", error)])
            }
            Status::Stopped { url } => fill(m.stopped, &[("url", url)]),
            Status::SavingPdf => m.saving_pdf.to_string(),
            Status::SavedPdf { path } => fill(m.saved_pdf, &[("path", path)]),
            Status::PdfCanceled => m.pdf_canceled.to_string(),
            Status::FailedPdf { error } => fill(m.failed_pdf, &[("error", error)]),
            Status::SavedHtml { path } => fill(m.saved_html, &[("path", path)]),
            Status::HtmlCanceled => m.html_canceled.to_string(),
            Status::FailedHtml { error } => fill(m.failed_html, &[("error", error)]),
            Status::NothingToSave => m.nothing_to_save.to_string(),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Status::Failed { .. } | Status::FailedPdf { .. } | Status::FailedHtml { .. }
        )
    }
}

#[derive(Debug)]
pub struct Tab {
    pub id: TabId,
    /// Page title from the generated HTML; empty for a fresh tab (localized fallback).
    pub title: String,
    /// SVG favicon as a data URI, extracted from the generated page.
    pub icon: Option<String>,
    /// Text shown in the address bar for this tab (may be an unsent draft).
    pub address: String,
    /// Absolute URL of the page currently shown; base for relative links.
    pub url: String,
    entries: Vec<Entry>,
    /// Position in `entries` of the page currently shown; None for a fresh tab.
    index: Option<usize>,
    /// Navigation generation: bumped by every navigation and by back/forward,
    /// so a late result sees a newer value and discards itself.
    nav_seq: u64,
    pending: Option<RequestId>,
    pub status: Status,
    pub content: Content,
    /// Bumped whenever `content` changes, so shells reload the webview only then.
    pub content_seq: u64,
}

impl Tab {
    fn new(id: TabId) -> Tab {
        Tab {
            id,
            title: String::new(),
            icon: None,
            address: String::new(),
            url: String::new(),
            entries: Vec::new(),
            index: None,
            nav_seq: 0,
            pending: None,
            status: Status::Ready,
            content: Content::Empty,
            content_seq: 0,
        }
    }

    pub fn display_title(&self, m: &Messages) -> String {
        if self.title.is_empty() {
            m.new_tab.to_string()
        } else {
            self.title.clone()
        }
    }

    pub fn can_back(&self) -> bool {
        matches!(self.index, Some(i) if i > 0)
    }

    pub fn can_forward(&self) -> bool {
        matches!(self.index, Some(i) if i + 1 < self.entries.len())
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    pub fn pending_request(&self) -> Option<RequestId> {
        self.pending
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// SVG source for the tab icon: the page's own favicon, else a letter tile,
    /// else a neutral tile for a fresh tab.
    pub fn icon_svg(&self) -> String {
        if let Some(svg) = self.icon.as_deref().and_then(page::svg_from_data_uri) {
            return svg;
        }
        if !self.url.is_empty() {
            return page::letter_icon_svg(&self.url);
        }
        NEW_TAB_ICON.to_string()
    }

    /// Raw HTML of the current page, for saving.
    pub fn page_html(&self) -> Option<(&str, &str)> {
        match &self.content {
            Content::Page { url, html } => Some((url, html)),
            _ => None,
        }
    }

    fn set_content(&mut self, content: Content) {
        self.content = content;
        self.content_seq += 1;
    }

    /// Visited URLs up to the current page, oldest first — session context for the LLM.
    fn visited(&self) -> Vec<String> {
        let upto = self.index.map(|i| i + 1).unwrap_or(0);
        let start = upto.saturating_sub(HISTORY_LIMIT);
        self.entries[start..upto]
            .iter()
            .map(|e| e.url.clone())
            .collect()
    }
}

pub const NEW_TAB_ICON: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 16 16\">\
    <rect width=\"16\" height=\"16\" rx=\"3\" fill=\"#a1a1aa\"/></svg>";

/// A generation the shell must run through the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generation {
    pub tab: TabId,
    pub request: RequestId,
    pub site: SiteRequest,
}

/// What a navigation needs to know from settings.
#[derive(Debug, Clone, Copy)]
pub struct NavigateOptions<'a> {
    pub search_url: &'a str,
    /// Without a key (and outside mock mode) navigation fails fast with guidance.
    pub can_generate: bool,
    pub messages: &'a Messages,
}

#[derive(Debug, Clone)]
struct ClosedTab {
    entries: Vec<Entry>,
    index: Option<usize>,
    address: String,
}

#[derive(Debug)]
pub struct Browser {
    tabs: Vec<Tab>,
    active: TabId,
    next_tab_id: TabId,
    next_request_id: RequestId,
    closed: Vec<ClosedTab>,
}

impl Default for Browser {
    fn default() -> Self {
        Browser::new()
    }
}

impl Browser {
    /// A browser with one fresh, active tab.
    pub fn new() -> Browser {
        let mut browser = Browser {
            tabs: Vec::new(),
            active: 0,
            next_tab_id: 1,
            next_request_id: 1,
            closed: Vec::new(),
        };
        browser.new_tab();
        browser
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn active_id(&self) -> TabId {
        self.active
    }

    pub fn active(&self) -> &Tab {
        self.tab(self.active).expect("active tab exists")
    }

    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    pub fn new_tab(&mut self) -> TabId {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab::new(id));
        self.active = id;
        id
    }

    pub fn activate(&mut self, id: TabId) -> bool {
        if self.tab(id).is_some() {
            self.active = id;
            true
        } else {
            false
        }
    }

    /// Close a tab. Returns the request to cancel (if one was in flight) and the
    /// neighbour that became active; `None` when the last tab was closed.
    pub fn close_tab(&mut self, id: TabId) -> CloseOutcome {
        let Some(pos) = self.index_of(id) else {
            return CloseOutcome {
                cancelled: None,
                activated: None,
            };
        };
        let mut tab = self.tabs.remove(pos);
        let cancelled = tab.pending.take();
        if !tab.entries.is_empty() {
            self.closed.push(ClosedTab {
                entries: tab.entries,
                index: tab.index,
                address: tab.address,
            });
            if self.closed.len() > MAX_CLOSED {
                self.closed.remove(0);
            }
        }
        let activated = if self.tabs.is_empty() {
            None
        } else if self.active == id {
            let neighbour = self.tabs[pos.min(self.tabs.len() - 1)].id;
            self.active = neighbour;
            Some(neighbour)
        } else {
            Some(self.active)
        };
        CloseOutcome {
            cancelled,
            activated,
        }
    }

    /// How many closed tabs can be reopened.
    pub fn closed_count(&self) -> usize {
        self.closed.len()
    }

    /// Bring back the most recently closed tab with its history.
    pub fn reopen_closed(&mut self) -> Option<TabId> {
        let closed = self.closed.pop()?;
        let id = self.new_tab();
        let tab = self.tab_mut(id).expect("just created");
        tab.entries = closed.entries;
        tab.address = closed.address;
        if let Some(index) = closed.index {
            show_entry(tab, index);
        }
        Some(id)
    }

    /// Keep in-progress typing per tab so switching tabs and back restores the draft.
    pub fn set_address(&mut self, id: TabId, text: &str) {
        if let Some(tab) = self.tab_mut(id) {
            tab.address = text.to_string();
        }
    }

    pub fn set_status(&mut self, id: TabId, status: Status) {
        if let Some(tab) = self.tab_mut(id) {
            tab.status = status;
        }
    }

    /// Navigate from address-bar text. Returns the generation to run, or `None`
    /// when nothing was started (empty input, or refused for lack of a key —
    /// then the tab already shows why).
    pub fn navigate(
        &mut self,
        id: TabId,
        input: &str,
        opts: NavigateOptions,
    ) -> Option<Generation> {
        let input = input.trim();
        let url = resolve_input(input, opts.search_url).ok()?;
        self.start(id, input.to_string(), url, opts)
    }

    /// Navigate to an absolute URL (link target, retry, blocked navigation).
    pub fn navigate_url(
        &mut self,
        id: TabId,
        url: &str,
        opts: NavigateOptions,
    ) -> Option<Generation> {
        let url = normalize_url(url).ok()?;
        self.start(id, url.clone(), url, opts)
    }

    /// A link (or form) in the page: resolve `href` against the tab's page.
    /// In-page anchors and non-web schemes are ignored.
    pub fn open_link(
        &mut self,
        id: TabId,
        href: &str,
        opts: NavigateOptions,
    ) -> Option<Generation> {
        let base = self.tab(id).map(|t| t.url.clone()).unwrap_or_default();
        let target = resolve_link(&base, href)?;
        self.navigate_url(id, &target, opts)
    }

    /// Regenerate the current page (or retry a failed one).
    pub fn reload(&mut self, id: TabId, opts: NavigateOptions) -> Option<Generation> {
        let tab = self.tab(id)?;
        let url = match &tab.content {
            Content::Page { url, .. } | Content::Error { url, .. } => url.clone(),
            Content::Empty => return None,
        };
        let address = tab.address.clone();
        self.start(id, address, url, opts)
    }

    fn start(
        &mut self,
        id: TabId,
        address: String,
        url: String,
        opts: NavigateOptions,
    ) -> Option<Generation> {
        let request = self.next_request_id;
        let can_generate = opts.can_generate;
        let no_key_message = opts.messages.error_no_api_key.to_string();
        let tab = self.tab_mut(id)?;
        // A new navigation supersedes whatever is still generating for this tab.
        tab.nav_seq += 1;
        tab.pending = None;
        tab.address = address;
        if !can_generate {
            tab.status = Status::Failed {
                url: url.clone(),
                error: no_key_message.clone(),
            };
            tab.set_content(Content::Error {
                url,
                message: no_key_message,
                offer_settings: true,
            });
            return None;
        }
        tab.pending = Some(request);
        tab.status = Status::Loading { url: url.clone() };
        let history = tab.visited();
        let referer = history.last().cloned();
        self.next_request_id += 1;
        Some(Generation {
            tab: id,
            request,
            site: SiteRequest {
                url,
                referer,
                history,
            },
        })
    }

    /// Stop the in-flight generation of a tab. Returns the request to abort.
    pub fn cancel(&mut self, id: TabId) -> Option<RequestId> {
        let tab = self.tab_mut(id)?;
        let request = tab.pending.take()?;
        tab.nav_seq += 1;
        if let Status::Loading { url } = &tab.status {
            tab.status = Status::Stopped { url: url.clone() };
        }
        Some(request)
    }

    /// Apply a finished generation. Returns false when it was superseded.
    pub fn finish(
        &mut self,
        id: TabId,
        request: RequestId,
        result: Result<String, GenerateError>,
    ) -> bool {
        let Some(tab) = self.tab_mut(id) else {
            return false;
        };
        if tab.pending != Some(request) {
            return false;
        }
        tab.pending = None;
        let url = match &tab.status {
            Status::Loading { url } => url.clone(),
            _ => tab.url.clone(),
        };
        match result {
            Ok(html) => {
                tab.url = url.clone();
                tab.title = page::extract_title(&html).unwrap_or_else(|| url.clone());
                tab.icon = page::extract_icon(&html);
                // Navigating from mid-history discards the forward entries, like a real browser.
                let keep = tab.index.map(|i| i + 1).unwrap_or(0);
                tab.entries.truncate(keep);
                tab.entries.push(Entry {
                    url: url.clone(),
                    address: tab.address.clone(),
                    title: tab.title.clone(),
                    icon: tab.icon.clone(),
                    html: html.clone(),
                });
                if tab.entries.len() > MAX_ENTRIES {
                    tab.entries.remove(0);
                }
                tab.index = Some(tab.entries.len() - 1);
                tab.status = Status::Loaded {
                    url: tab.address.clone(),
                };
                tab.set_content(Content::Page { url, html });
            }
            Err(error) => {
                let message = error.to_string();
                tab.status = Status::Failed {
                    url: tab.address.clone(),
                    error: message.clone(),
                };
                tab.set_content(Content::Error {
                    url,
                    message,
                    offer_settings: error.is_configuration(),
                });
            }
        }
        true
    }

    /// Show a history entry: restore its cached page without regenerating.
    pub fn back(&mut self, id: TabId) -> Option<RequestId> {
        let tab = self.tab_mut(id)?;
        let index = tab.index?.checked_sub(1)?;
        let cancelled = abandon_pending(tab);
        show_entry(tab, index);
        cancelled
    }

    pub fn forward(&mut self, id: TabId) -> Option<RequestId> {
        let tab = self.tab_mut(id)?;
        let index = tab.index? + 1;
        if index >= tab.entries.len() {
            return None;
        }
        let cancelled = abandon_pending(tab);
        show_entry(tab, index);
        cancelled
    }
}

/// Result of closing a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloseOutcome {
    /// Generation that was still in flight for the closed tab.
    pub cancelled: Option<RequestId>,
    /// Tab that is active now; `None` when no tabs are left.
    pub activated: Option<TabId>,
}

/// Moving through history abandons any generation still in flight.
fn abandon_pending(tab: &mut Tab) -> Option<RequestId> {
    let pending = tab.pending.take();
    if pending.is_some() {
        tab.nav_seq += 1;
    }
    pending
}

fn show_entry(tab: &mut Tab, index: usize) {
    let Some(entry) = tab.entries.get(index).cloned() else {
        return;
    };
    tab.index = Some(index);
    tab.url = entry.url.clone();
    tab.address = entry.address.clone();
    tab.title = entry.title;
    tab.icon = entry.icon;
    tab.status = Status::Loaded { url: entry.address };
    tab.set_content(Content::Page {
        url: entry.url,
        html: entry.html,
    });
}

/// Resolve a link target against the current page. `None` for in-page anchors,
/// non-web schemes and unresolvable hrefs.
pub fn resolve_link(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() || href.to_ascii_lowercase().starts_with("javascript:") {
        return None;
    }
    let target = match url::Url::parse(base) {
        Ok(base_url) => base_url.join(href).ok()?,
        Err(_) => url::Url::parse(href).ok()?,
    };
    if !matches!(target.scheme(), "http" | "https") {
        return None;
    }
    // Hash-only change on the current page: in-page scroll, not a new document.
    if target.fragment().is_some() {
        let mut without = target.clone();
        without.set_fragment(None);
        let mut base_without = url::Url::parse(base).ok();
        if let Some(b) = base_without.as_mut() {
            b.set_fragment(None);
        }
        if base_without.map(|b| b == without).unwrap_or(false) {
            return None;
        }
    }
    Some(target.to_string())
}

#[cfg(test)]
mod tests;
