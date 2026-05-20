use objc2::rc::Retained as Id;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSOpenPanel, NSOpenSavePanelDelegate};
use objc2_foundation::{NSObject, NSObjectProtocol, NSURL};

use super::{NSOpenPanelExt, NSURLExt};
use crate::dialog::FileFiltersBag;

pub struct OpenPanelDelegateIvars {
    filters: FileFiltersBag,
}

define_class! {
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = OpenPanelDelegateIvars]
    pub struct OpenPanelDelegate;

    unsafe impl NSObjectProtocol for OpenPanelDelegate {}

    unsafe impl NSOpenSavePanelDelegate for OpenPanelDelegate {
        #[unsafe(method(panel:shouldEnableURL:))]
        unsafe fn should_enable_url(&self, _sender: &NSOpenPanel, url: &NSURL) -> bool {
            self.selectable(url)
        }
    }
}

impl OpenPanelDelegate {
    pub fn attach(panel: &NSOpenPanel, filters: &FileFiltersBag) -> Id<Self> {
        let ivars = OpenPanelDelegateIvars {
            filters: filters.to_owned(),
        };

        let this = Self::alloc(panel.mtm()).set_ivars(ivars);
        let this: Id<Self> = unsafe { msg_send![super(this), init] };

        panel.set_delegate(&this);

        this
    }

    fn selectable(&self, url: &NSURL) -> bool {
        if url.hasDirectoryPath() {
            return true;
        }

        let Some(path) = url.to_path_buf() else {
            return false;
        };

        self.ivars().filters.accepts(&path)
    }
}
