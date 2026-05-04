use std::cell::Cell;
use std::path::PathBuf;

use objc2::rc::Retained as Id;
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSColor, NSImage, NSImageNameCaution,
    NSLayoutConstraintOrientation, NSOpenSavePanelDelegate, NSPopUpButton, NSSavePanel,
    NSStackView, NSStackViewGravity, NSTextField, NSView,
};
use objc2_foundation::{
    NSArray, NSEdgeInsets, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

use super::{NSAlertExt, NSPopUpButtonExt, NSSavePanelExt, NSTextFieldExt};
use crate::dialog::{FileFilter, FileFiltersBag};

pub struct SavePanelDelegateIvars {
    accessory: Cell<Option<Id<NSView>>>,
    filters: FileFiltersBag,
    selected: Cell<usize>,
}

define_class! {
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = SavePanelDelegateIvars]
    pub struct SavePanelDelegate;

    unsafe impl NSObjectProtocol for SavePanelDelegate {}

    unsafe impl NSOpenSavePanelDelegate for SavePanelDelegate {
        #[unsafe(method_id(panel:userEnteredFilename:confirmed:))]
        unsafe fn user_entered_filename(
            &self,
            sender: &NSSavePanel,
            filename: &NSString,
            _ok_flag: bool,
        ) -> Option<Id<NSString>> {
            unsafe { self.validate(sender, filename) }
        }
    }

    impl SavePanelDelegate {
        #[unsafe(method(onItemSelected:))]
        unsafe fn on_item_selected(&self, sender: &NSPopUpButton) {
            let index = sender.indexOfSelectedItem();
            self.ivars().selected.set(index as usize);
        }
    }
}

impl SavePanelDelegate {
    pub fn attach(panel: &NSSavePanel, filters: &FileFiltersBag) -> Id<Self> {
        let ivars = SavePanelDelegateIvars {
            accessory: Cell::new(None),
            filters: filters.to_owned(),
            selected: Cell::new(0),
        };

        let this = Self::alloc(panel.mtm()).set_ivars(ivars);
        let this: Id<Self> = unsafe { msg_send![super(this), init] };

        panel.set_delegate(&this);

        // If there are filters specified, show a dropdown on the panel
        if !filters.items.is_empty() {
            let accessory = this.create_accessory(filters);
            panel.setAccessoryView(Some(&accessory));
            this.ivars().accessory.set(Some(accessory));
        }

        this
    }

    fn selected_filter(&self) -> Option<&FileFilter> {
        let ivars = self.ivars();
        ivars.filters.items.get(ivars.selected.get())
    }

    fn create_accessory(&self, filters: &FileFiltersBag) -> Id<NSView> {
        let titles = self.format_titles(filters);

        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(480.0, 0.0));
        let dropdown = NSPopUpButton::new_with_frame(self.mtm(), frame);
        dropdown.addItemsWithTitles(&titles);
        dropdown.selectItemAtIndex(0);
        dropdown.set_action(sel!(onItemSelected:));
        dropdown.set_target(self);

        let label = NSTextField::label_with_string(self.mtm(), "File Type: ");
        label.set_text_color(&NSColor::secondaryLabelColor());

        let stack = NSStackView::new(self.mtm());
        // Edge insets in specific axis are only enforced when hugging priority >= 500
        // See https://stackoverflow.com/questions/54533509/nsstackview-edgeinsets-gets-ignored
        stack.setHuggingPriority_forOrientation(500.0, NSLayoutConstraintOrientation::Vertical);
        stack.setHuggingPriority_forOrientation(500.0, NSLayoutConstraintOrientation::Horizontal);
        stack.setEdgeInsets(NSEdgeInsets {
            top: 16.0,
            left: 20.0,
            bottom: 16.0,
            right: 20.0,
        });
        stack.addView_inGravity(&label, NSStackViewGravity::Center);
        stack.addView_inGravity(&dropdown, NSStackViewGravity::Center);

        stack.into_super()
    }

    fn format_titles(&self, filters: &FileFiltersBag) -> Id<NSArray<NSString>> {
        filters
            .items
            .iter()
            .map(|filter| filter.format("{name} ({types})", "*{ext}", " "))
            .map(|title| NSString::from_str(&title))
            .collect()
    }

    unsafe fn validate(&self, panel: &NSSavePanel, filename: &NSString) -> Option<Id<NSString>> {
        let Some(filter) = self.selected_filter() else {
            return Some(filename.retain());
        };

        let path = PathBuf::from(filename.to_string());
        if filter.accepts(&path) {
            return Some(filename.retain());
        }

        if path.extension().is_none() {
            let ext = filter.extensions.first().unwrap();
            return Some(NSString::from_str(&format!("{filename}{ext}")));
        }

        let explain = format!("Filename \"{}\" is not of type {}.", filename, filter.name);

        let alert = NSAlert::new(self.mtm());
        alert.set_message_text("Unrecognized File Type");
        alert.set_informative_text(&explain);
        unsafe { alert.setIcon(NSImage::imageNamed(NSImageNameCaution).as_deref()) };

        let confirm = alert.add_button("Continue Anyway");
        let cancel = alert.add_button("Cancel");
        confirm.setKeyEquivalent(&NSString::from_str(""));
        cancel.setKeyEquivalent(&NSString::from_str("\r"));

        let response = alert.run(Some(panel));
        if response == NSAlertFirstButtonReturn {
            return Some(filename.retain());
        }

        None
    }
}
