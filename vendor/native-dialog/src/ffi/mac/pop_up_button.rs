use objc2::rc::Retained as Id;
use objc2::runtime::Sel;
use objc2_app_kit::NSPopUpButton;
use objc2_foundation::{MainThreadMarker, NSObject, NSRect};

pub trait NSPopUpButtonExt {
    fn new_with_frame(mtm: MainThreadMarker, frame: NSRect) -> Id<Self>;
    fn set_action(&self, sel: Sel);
    fn set_target(&self, target: &NSObject);
}

impl NSPopUpButtonExt for NSPopUpButton {
    fn new_with_frame(mtm: MainThreadMarker, frame: NSRect) -> Id<Self> {
        NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), frame, false)
    }

    fn set_action(&self, sel: Sel) {
        unsafe { self.setAction(Some(sel)) }
    }

    fn set_target(&self, target: &NSObject) {
        unsafe { self.setTarget(Some(target)) }
    }
}
