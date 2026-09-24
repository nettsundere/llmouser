//! The About window: centered icon, name, version and copyright. A plain
//! window (not the system panel) so the layout and icon are identical in
//! development and packaged builds and in every language.

use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSFont, NSImage, NSImageView, NSLayoutAttribute,
    NSLayoutConstraint, NSStackView, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
    NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSArray, NSData};

use llmouser_browser::automation::AboutInfo;
use llmouser_browser::i18n::fill;
use llmouser_browser::{APP_COPYRIGHT, APP_NAME, APP_VERSION};

use super::app::{state, ICON_PNG};
use super::util::{ns, rect};

define_class!(
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "LLMouserAboutWindow"]
    #[ivars = ()]
    pub struct AboutPanel;

    impl AboutPanel {
        /// Escape closes, like the system about panel.
        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.close();
        }
    }
);

impl AboutPanel {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;
        unsafe {
            msg_send![
                super(this),
                initWithContentRect: rect(0.0, 0.0, 300.0, 330.0),
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        }
    }
}

pub struct AboutWindow {
    pub window: Retained<AboutPanel>,
    pub icon_view: Retained<NSImageView>,
    name: Retained<NSTextField>,
    version: Retained<NSTextField>,
    copyright: Retained<NSTextField>,
}

impl AboutWindow {
    pub fn new(mtm: MainThreadMarker) -> Rc<AboutWindow> {
        let window = AboutPanel::new(mtm);
        unsafe { window.setReleasedWhenClosed(false) };

        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(ICON_PNG));
        let icon_view = match &image {
            Some(img) => NSImageView::imageViewWithImage(img, mtm),
            None => {
                NSImageView::initWithFrame(NSImageView::alloc(mtm), rect(0.0, 0.0, 128.0, 128.0))
            }
        };
        icon_view
            .widthAnchor()
            .constraintEqualToConstant(128.0)
            .setActive(true);
        icon_view
            .heightAnchor()
            .constraintEqualToConstant(128.0)
            .setActive(true);

        let name = NSTextField::labelWithString(&ns(APP_NAME), mtm);
        name.setFont(Some(&NSFont::boldSystemFontOfSize(16.0)));
        let version = NSTextField::labelWithString(&ns(""), mtm);
        version.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        version.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let copyright = NSTextField::labelWithString(&ns(APP_COPYRIGHT), mtm);
        copyright.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        copyright.setTextColor(Some(&NSColor::secondaryLabelColor()));

        let stack = NSStackView::stackViewWithViews(
            &NSArray::from_slice(&[
                &*icon_view as &NSView,
                &*name as &NSView,
                &*version as &NSView,
                &*copyright as &NSView,
            ]),
            mtm,
        );
        stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
        stack.setAlignment(NSLayoutAttribute::CenterX);
        stack.setSpacing(6.0);
        stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        let content = window.contentView().expect("content view");
        content.addSubview(&stack);
        NSLayoutConstraint::activateConstraints(&NSArray::from_retained_slice(&[
            stack
                .centerXAnchor()
                .constraintEqualToAnchor(&content.centerXAnchor()),
            stack
                .centerYAnchor()
                .constraintEqualToAnchor(&content.centerYAnchor()),
        ]));

        let about = Rc::new(AboutWindow {
            window,
            icon_view,
            name,
            version,
            copyright,
        });
        about.retranslate();
        about
    }

    pub fn show(&self) {
        self.retranslate();
        if !self.window.isVisible() {
            self.window.center();
        }
        self.window.makeKeyAndOrderFront(None);
    }

    pub fn close(&self) {
        self.window.close();
    }

    pub fn retranslate(&self) {
        let m = state().session.borrow().messages();
        self.window
            .setTitle(&ns(&fill(m.menu_about, &[("app", APP_NAME)])));
        self.version
            .setStringValue(&ns(&format!("{} {}", m.about_version, APP_VERSION)));
    }

    pub fn info(&self) -> AboutInfo {
        let content = self.window.contentView().expect("content view");
        content.layoutSubtreeIfNeeded();
        let icon_frame = self
            .icon_view
            .convertRect_toView(self.icon_view.bounds(), Some(&content));
        let icon_center = icon_frame.origin.x + icon_frame.size.width / 2.0;
        let content_center = content.bounds().size.width / 2.0;
        let icon_loaded = self
            .icon_view
            .image()
            .map(|i| i.isValid() && i.size().width > 0.0)
            .unwrap_or(false);
        AboutInfo {
            open: self.window.isVisible(),
            title: self.window.title().to_string(),
            name: self.name.stringValue().to_string(),
            version_line: self.version.stringValue().to_string(),
            copyright: self.copyright.stringValue().to_string(),
            icon_loaded,
            icon_off_center: (icon_center - content_center).abs(),
        }
    }
}
