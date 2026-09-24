//! Saving the current page as PDF (WebKit print operation to a file, no
//! dialog) or as HTML.

use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;
use webkit6 as webkit;

use llmouser_browser::{Status, TabId};

use super::app::{self, state};
use super::window::render;
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

pub fn save_pdf(tab: TabId) {
    save(tab, Kind::Pdf);
}

pub fn save_html(tab: TabId) {
    save(tab, Kind::Html);
}

fn save(tab: TabId, kind: Kind) {
    let page = state().session.borrow_mut().page_to_save(tab);
    let Some((url, _)) = page else {
        render(tab);
        return;
    };
    let suggested = default_file_name(&url, kind.extension());
    match Session::save_dialog_stub() {
        Some(Some(path)) => write(tab, kind, path),
        Some(None) => finish(tab, kind.cancelled()),
        None => {
            let dialog = gtk::FileDialog::new();
            dialog.set_initial_name(Some(&suggested));
            let window = app::main_window().window.clone();
            dialog.save(
                Some(&window),
                None::<&gio::Cancellable>,
                move |result| match result.ok().and_then(|f| f.path()) {
                    Some(path) => write(tab, kind, path),
                    None => finish(tab, kind.cancelled()),
                },
            );
        }
    }
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
            let w = app::main_window();
            let Some(entry) = w.entry_for(tab) else {
                return;
            };
            finish(tab, Status::SavingPdf);
            let op = webkit::PrintOperation::new(&entry.webview);
            let settings = gtk::PrintSettings::new();
            settings.set(gtk::PRINT_SETTINGS_PRINTER, Some("Print to File"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            settings.set(
                gtk::PRINT_SETTINGS_OUTPUT_URI,
                Some(&format!("file://{}", path.display())),
            );
            op.set_print_settings(&settings);
            let done_path = path.clone();
            op.connect_finished(move |_| {
                let written = done_path.exists();
                finish(
                    tab,
                    if written {
                        Status::SavedPdf {
                            path: done_path.display().to_string(),
                        }
                    } else {
                        Status::FailedPdf {
                            error: "print operation produced no file".into(),
                        }
                    },
                );
                state().pending_print.borrow_mut().take();
            });
            op.connect_failed(move |_, error| {
                finish(
                    tab,
                    Status::FailedPdf {
                        error: error.to_string(),
                    },
                );
                state().pending_print.borrow_mut().take();
            });
            *state().pending_print.borrow_mut() = Some(op.clone());
            op.print();
        }
    }
}

fn finish(tab: TabId, status: Status) {
    state().session.borrow_mut().set_status(tab, status);
    render(tab);
}
