use std::path::{Path, PathBuf};

use objc2::rc::Retained as Id;
use objc2_foundation::{NSString, NSURL};

pub trait NSURLExt {
    fn new_path(s: &Path) -> Id<Self>;
    fn to_path_buf(&self) -> Option<PathBuf>;
}

impl NSURLExt for NSURL {
    fn new_path(s: &Path) -> Id<Self> {
        let s = NSString::from_str(&s.to_string_lossy());
        NSURL::fileURLWithPath(&s)
    }

    fn to_path_buf(&self) -> Option<PathBuf> {
        self.absoluteURL()
            .and_then(|x| x.path())
            .map(|x| x.to_string().into())
    }
}
