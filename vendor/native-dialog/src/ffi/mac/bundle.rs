use objc2::rc::Retained as Id;
use objc2_app_kit::{NSBundleImageExtension, NSImage};
use objc2_foundation::{NSBundle, NSString};

pub trait NSBundleExt {
    fn from_path(path: &str) -> Option<Id<Self>>;
    fn image(&self, name: &str) -> Option<Id<NSImage>>;
}

impl NSBundleExt for NSBundle {
    fn from_path(path: &str) -> Option<Id<Self>> {
        let path = NSString::from_str(path);
        NSBundle::bundleWithPath(&path)
    }

    fn image(&self, name: &str) -> Option<Id<NSImage>> {
        let name = NSString::from_str(name);
        self.imageForResource(&name)
    }
}
