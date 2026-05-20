use dispatch2::run_on_main;
use objc2::MainThreadMarker;
use objc2::rc::Retained as Id;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn};

use crate::Result;
use crate::dialog::{DialogImpl, MessageAlert, MessageConfirm};
use crate::ffi::mac::NSAlertExt;

impl MessageAlert {
    fn create(&self, mtm: MainThreadMarker) -> Id<NSAlert> {
        let alert = NSAlert::new(mtm);

        alert.set_informative_text(&self.text);
        alert.set_message_text(&self.title);
        alert.set_level_icon(self.level);

        alert
    }
}

impl DialogImpl for MessageAlert {
    fn show(self) -> Result<Self::Output> {
        run_on_main(|mtm| {
            let alert = self.create(mtm);
            alert.show(self.owner)
        });

        Ok(())
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        use crate::ffi::mac::NSAlertAsyncExt;

        let res = run_on_main(|mtm| {
            let alert = self.create(mtm);
            alert.spawn(self.owner)
        });

        res.await;
        Ok(())
    }
}

impl MessageConfirm {
    fn create(&self, mtm: MainThreadMarker) -> Id<NSAlert> {
        let alert = NSAlert::new(mtm);

        alert.set_informative_text(&self.text);
        alert.set_message_text(&self.title);
        alert.set_level_icon(self.level);

        alert.add_button("Yes");
        alert.add_button("No");

        alert
    }
}

impl DialogImpl for MessageConfirm {
    fn show(self) -> Result<Self::Output> {
        let res = run_on_main(|mtm| {
            let alert = self.create(mtm);
            alert.show(self.owner)
        });

        Ok(res == NSAlertFirstButtonReturn)
    }

    #[cfg(feature = "async")]
    async fn spawn(self) -> Result<Self::Output> {
        use crate::ffi::mac::NSAlertAsyncExt;

        let res = run_on_main(|mtm| {
            let alert = self.create(mtm);
            alert.spawn(self.owner)
        });

        Ok(res.await == NSAlertFirstButtonReturn)
    }
}
