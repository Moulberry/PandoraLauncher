use std::path::PathBuf;

use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};

use super::{DispatchResponse, NSSavePanelAsyncExt, NSURLExt};
use crate::ffi::UnsafeWindowHandle;

pub trait NSOpenPanelAsyncExt {
    fn spawn(&self, owner: UnsafeWindowHandle) -> DispatchResponse<Vec<PathBuf>>;
}

impl NSOpenPanelAsyncExt for NSOpenPanel {
    fn spawn(&self, owner: UnsafeWindowHandle) -> DispatchResponse<Vec<PathBuf>> {
        let owner = unsafe { owner.as_appkit() };

        self.begin(owner.as_deref(), move |panel, response| {
            let panel = panel.downcast_ref::<NSOpenPanel>().unwrap();

            (response == NSModalResponseOK)
                .then(|| panel.URLs())
                .map(|urls| urls.into_iter().filter_map(|x| x.to_path_buf()).collect())
                .unwrap_or_default()
        })
    }
}
