use block2::RcBlock;
use objc2::Message;
use objc2_app_kit::{
    NSAlert, NSApplication, NSApplicationActivationPolicy, NSModalResponse, NSSavePanel, NSWindow,
};

pub trait NSApplicationExt {
    fn run_modal<T: SheetModal>(&self, modal: &T) -> NSModalResponse;
    fn run_sheet<T: SheetModal>(&self, window: &NSWindow, sheet: &T) -> NSModalResponse;
}

impl NSApplicationExt for NSApplication {
    fn run_modal<T: SheetModal>(&self, modal: &T) -> NSModalResponse {
        let policy = self.activationPolicy();

        self.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        let response = modal.run_modal_event_loop(self);
        self.setActivationPolicy(policy);

        response
    }

    fn run_sheet<T: SheetModal>(&self, window: &NSWindow, sheet: &T) -> NSModalResponse {
        sheet.present_sheet(self, window);
        sheet.run_modal_event_loop(self)
    }
}

/// By default, sheets run asynchronously on the main event loop, so we cannot get their
/// responses synchronously. If we use something like channels to wait for it, they will
/// block the event loop and make the entire UI frozen (effectively a deadlock). However,
/// AppKit provides an way to run "modal event loops", which is another event loop that
/// runs in the main event loop. It still blocks the main event loop, but it takes the
/// control of the sheet UI, doing all the jobs necessary to react to the UI events from
/// the sheet. Therefore, we can utilize it to wait for responses of sheets synchronously
/// but still allow users to operate on the UI.
pub trait SheetModal {
    fn present_sheet(&self, app: &NSApplication, window: &NSWindow);
    fn run_modal_event_loop(&self, app: &NSApplication) -> NSModalResponse;
}

impl SheetModal for NSAlert {
    fn present_sheet(&self, app: &NSApplication, window: &NSWindow) {
        let handler = modal_completion(app);
        self.beginSheetModalForWindow_completionHandler(window, Some(&handler))
    }

    fn run_modal_event_loop(&self, _: &NSApplication) -> NSModalResponse {
        self.runModal()
    }
}

impl SheetModal for NSSavePanel {
    fn present_sheet(&self, app: &NSApplication, window: &NSWindow) {
        let handler = modal_completion(app);
        self.beginSheetModalForWindow_completionHandler(window, &handler);
    }

    fn run_modal_event_loop(&self, _: &NSApplication) -> NSModalResponse {
        self.runModal()
    }
}

fn modal_completion(app: &NSApplication) -> RcBlock<dyn Fn(NSModalResponse)> {
    let app = app.retain();
    RcBlock::new(move |response| app.stopModalWithCode(response))
}
