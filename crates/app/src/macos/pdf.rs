//! Saving the current page: as a PDF through the print system (no dialog, no
//! hidden windows: the live webview prints itself), or as an HTML file.

use std::path::PathBuf;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{sel, AnyThread};
use objc2_app_kit::{
    NSModalResponse, NSModalResponseOK, NSPrintInfo, NSPrintJobDisposition, NSPrintJobSavingURL,
    NSPrintSaveJob, NSPrintingPaginationMode, NSSavePanel,
};
use objc2_foundation::{NSArray, NSMutableDictionary, NSString, NSURL};

use llmouser_browser::{Status, TabId};

use super::app::state;
use super::util::{ns, rect};
use super::window::{self, TabWindow};
use crate::session::{default_file_name, Session};

#[derive(Clone, Copy)]
enum Kind {
    Pdf,
    Html,
}

impl Kind {
    fn extension(self) -> &'static str {
        match self {
            Kind::Pdf => "pdf",
            Kind::Html => "html",
        }
    }

    fn cancelled(self) -> Status {
        match self {
            Kind::Pdf => Status::PdfCanceled,
            Kind::Html => Status::HtmlCanceled,
        }
    }
}

pub fn save_pdf(w: &Rc<TabWindow>) {
    save(w, Kind::Pdf);
}

pub fn save_html(w: &Rc<TabWindow>) {
    save(w, Kind::Html);
}

fn save(w: &Rc<TabWindow>, kind: Kind) {
    let tab = w.tab;
    let page = state().session.borrow_mut().page_to_save(tab);
    let Some((url, _)) = page else {
        window::render(tab);
        return;
    };
    let suggested = default_file_name(&url, kind.extension());
    match Session::save_dialog_stub() {
        Some(Some(path)) => write(tab, kind, path),
        Some(None) => finish(tab, kind.cancelled()),
        None => ask(w, kind, &suggested),
    }
}

fn ask(w: &Rc<TabWindow>, kind: Kind, suggested: &str) {
    let mtm = state().mtm;
    let panel = NSSavePanel::savePanel(mtm);
    panel.setNameFieldStringValue(&ns(suggested));
    panel.setCanCreateDirectories(true);
    #[allow(deprecated)]
    panel.setAllowedFileTypes(Some(&NSArray::from_retained_slice(&[ns(kind.extension())])));
    let tab = w.tab;
    let panel_ref: Retained<NSSavePanel> = panel.clone();
    let block = RcBlock::new(move |response: NSModalResponse| {
        let chosen = (response == NSModalResponseOK)
            .then(|| {
                panel_ref
                    .URL()
                    .and_then(|u| u.path())
                    .map(|p| PathBuf::from(p.to_string()))
            })
            .flatten();
        match chosen {
            Some(path) => write(tab, kind, path),
            None => finish(tab, kind.cancelled()),
        }
    });
    panel.beginSheetModalForWindow_completionHandler(&w.window, &block);
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
            let Some(w) = window::find(tab) else { return };
            finish(tab, Status::SavingPdf);
            print_to_pdf(&w, path);
        }
    }
}

fn finish(tab: TabId, status: Status) {
    state().session.borrow_mut().set_status(tab, status);
    window::render(tab);
}

/// Print the live webview to a PDF file without any panel.
fn print_to_pdf(w: &Rc<TabWindow>, path: PathBuf) {
    let st = state();
    let tab = w.tab;
    let file_url = NSURL::fileURLWithPath(&ns(&path.display().to_string()));
    let attributes: Retained<NSMutableDictionary<NSString, AnyObject>> = NSMutableDictionary::new();
    unsafe {
        let disposition: &NSString = NSPrintSaveJob;
        attributes.insert(NSPrintJobDisposition, disposition);
        attributes.insert(NSPrintJobSavingURL, &file_url);
    }
    let info = unsafe { NSPrintInfo::initWithDictionary(NSPrintInfo::alloc(), &attributes) };
    info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
    info.setVerticalPagination(NSPrintingPaginationMode::Automatic);
    info.setTopMargin(36.0);
    info.setBottomMargin(36.0);
    info.setLeftMargin(36.0);
    info.setRightMargin(36.0);
    info.setHorizontallyCentered(false);
    info.setVerticallyCentered(false);

    let op = unsafe { w.webview.printOperationWithPrintInfo(&info) };
    op.setShowsPrintPanel(false);
    op.setShowsProgressPanel(false);
    op.setJobTitle(Some(&w.window.title()));
    // WebKit's print view starts with an empty frame: give it the printable area.
    if let Some(view) = op.view() {
        let paper = info.paperSize();
        view.setFrame(rect(0.0, 0.0, paper.width - 72.0, paper.height - 72.0));
    }
    *st.pending_pdf.borrow_mut() = Some((tab, path));
    unsafe {
        op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
            &w.window,
            Some(st.delegate.as_target()),
            Some(sel!(printOperationDidRun:success:contextInfo:)),
            std::ptr::null_mut(),
        );
    }
}

/// Completion of the print job, delivered to the app delegate. AppKit may
/// deliver it off the main thread, so hop first: the state is main-thread only.
pub fn print_finished(success: bool) {
    if objc2::MainThreadMarker::new().is_none() {
        super::util::on_main(move || print_finished(success));
        return;
    }
    let pending = state().pending_pdf.borrow_mut().take();
    let Some((tab, path)) = pending else { return };
    let written = success && path.exists();
    let status = if written {
        Status::SavedPdf {
            path: path.display().to_string(),
        }
    } else {
        Status::FailedPdf {
            error: "print operation failed".into(),
        }
    };
    finish(tab, status);
}
