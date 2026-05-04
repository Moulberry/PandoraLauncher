use std::cell::Cell;

use block2::RcBlock;
use objc2::Message;
use objc2_app_kit::{NSAlert, NSModalResponse, NSWindow};

use super::DispatchResponse;
use crate::ffi::UnsafeWindowHandle;

pub trait NSAlertAsyncExt {
    fn spawn(&self, owner: UnsafeWindowHandle) -> DispatchResponse<NSModalResponse>;

    fn begin<T, F>(&self, owner: Option<&NSWindow>, callback: F) -> DispatchResponse<T>
    where
        T: Default + Send + 'static,
        F: Fn(&NSAlert, NSModalResponse) -> T + Send + 'static;
}

impl NSAlertAsyncExt for NSAlert {
    fn spawn(&self, owner: UnsafeWindowHandle) -> DispatchResponse<NSModalResponse> {
        let owner = unsafe { owner.as_appkit() };
        self.begin(owner.as_deref(), move |_, response| response)
    }

    fn begin<T, F>(&self, owner: Option<&NSWindow>, callback: F) -> DispatchResponse<T>
    where
        T: Default + Send + 'static,
        F: Fn(&NSAlert, NSModalResponse) -> T + Send + 'static,
    {
        let (send, recv) = futures_channel::oneshot::channel();

        let cell = Cell::new(Some(send));
        let alert = self.retain();
        let handler = move |response: NSModalResponse| {
            if let Some(send) = cell.take() {
                let _ = send.send(callback(&alert, response));
            }
        };

        match owner {
            Some(window) => {
                let block = RcBlock::new(handler);
                self.beginSheetModalForWindow_completionHandler(window, Some(&block))
            }
            None => handler(self.runModal()),
        }

        DispatchResponse::new(recv)
    }
}
