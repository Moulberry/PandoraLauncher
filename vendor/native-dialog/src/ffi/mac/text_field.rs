use objc2::rc::Retained as Id;
use objc2_app_kit::{NSColor, NSTextField};
use objc2_foundation::{MainThreadMarker, NSString};

pub trait NSTextFieldExt {
    fn label_with_string(mtm: MainThreadMarker, string: &str) -> Id<Self>;
    fn set_text_color(&self, color: &NSColor);
}

impl NSTextFieldExt for NSTextField {
    fn label_with_string(mtm: MainThreadMarker, string: &str) -> Id<Self> {
        let string = NSString::from_str(string);
        NSTextField::labelWithString(&string, mtm)
    }

    fn set_text_color(&self, color: &NSColor) {
        self.setTextColor(Some(color))
    }
}
