use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSImage;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// Identity of an Objective-C object, for comparing senders.
pub fn obj_ptr<T>(r: &T) -> *const AnyObject {
    (r as *const T).cast()
}

pub fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

pub fn size(w: f64, h: f64) -> NSSize {
    NSSize::new(w, h)
}

/// Run a closure on the main thread, later.
pub fn on_main(f: impl FnOnce() + Send + 'static) {
    dispatch2::DispatchQueue::main().exec_async(f);
}

/// An SF Symbol image.
pub fn symbol(name: &str, description: &str) -> Option<Retained<NSImage>> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns(name), Some(&ns(description)))
}
