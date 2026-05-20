use objc2::rc::Retained as Id;
use objc2_app_kit::{NSView, NSWindow};
use raw_window_handle::AppKitWindowHandle;

pub trait NSWindowExt {
    fn from_raw(handle: AppKitWindowHandle) -> Option<Id<Self>>;
}

impl NSWindowExt for NSWindow {
    fn from_raw(handle: AppKitWindowHandle) -> Option<Id<Self>> {
        let view = handle.ns_view.as_ptr();
        let view = unsafe { Id::<NSView>::retain(view.cast()) };
        view.and_then(|x| x.window())
    }
}
