use std::path::PathBuf;

use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};

use super::{NSSavePanelExt, NSURLExt, OpenPanelDelegate};
use crate::ffi::UnsafeWindowHandle;

pub trait NSOpenPanelExt {
    fn show(&self, owner: UnsafeWindowHandle) -> Vec<PathBuf>;

    fn set_delegate(&self, delegate: &OpenPanelDelegate);
}

impl NSOpenPanelExt for NSOpenPanel {
    fn show(&self, owner: UnsafeWindowHandle) -> Vec<PathBuf> {
        let owner = unsafe { owner.as_appkit() };
        let response = self.run(owner.as_deref());

        (response == NSModalResponseOK)
            .then(|| self.URLs())
            .map(|urls| urls.into_iter().filter_map(|x| x.to_path_buf()).collect())
            .unwrap_or_default()
    }

    fn set_delegate(&self, delegate: &OpenPanelDelegate) {
        unsafe { self.setDelegate(Some(ProtocolObject::from_ref(delegate))) };
    }
}
