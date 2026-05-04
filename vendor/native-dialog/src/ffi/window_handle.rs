use raw_window_handle::HasWindowHandle;

#[derive(Debug, Clone, Default)]
pub struct UnsafeWindowHandle {
    #[cfg(target_os = "macos")]
    pub appkit: appkit::Handle,

    #[cfg(target_os = "windows")]
    pub win32: win32::Handle,

    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    pub x11: x11::Handle,
}

unsafe impl Send for UnsafeWindowHandle {}
unsafe impl Sync for UnsafeWindowHandle {}

impl UnsafeWindowHandle {
    pub fn new<W: HasWindowHandle>(window: &W) -> Self {
        let Ok(handle) = window.window_handle() else {
            return Self::default();
        };

        Self {
            #[cfg(target_os = "macos")]
            appkit: appkit::Handle::new(handle.as_raw()),

            #[cfg(target_os = "windows")]
            win32: win32::Handle::new(handle.as_raw()),

            #[cfg(all(
                unix,
                not(target_os = "macos"),
                not(target_os = "ios"),
                not(target_os = "android")
            ))]
            x11: x11::Handle::new(handle.as_raw()),
        }
    }

    /// SAFETY: must be called on the main thread
    #[cfg(target_os = "macos")]
    pub unsafe fn as_appkit(&self) -> Option<appkit::Inner> {
        self.appkit.get()
    }

    #[cfg(target_os = "windows")]
    pub unsafe fn as_win32(&self) -> Option<win32::Inner> {
        self.win32.get()
    }

    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    ))]
    pub unsafe fn as_x11(&self) -> Option<x11::Inner> {
        self.x11.get()
    }
}

#[cfg(target_os = "macos")]
mod appkit {
    use objc2::Message;
    use objc2::rc::Retained;
    use objc2_app_kit::NSWindow;
    use raw_window_handle::RawWindowHandle;

    use crate::ffi::mac::NSWindowExt;

    pub type Inner = Retained<NSWindow>;

    #[derive(Debug, Default)]
    pub struct Handle {
        inner: Option<Inner>,
    }

    impl Clone for Handle {
        fn clone(&self) -> Self {
            Self { inner: self.get() }
        }
    }

    impl Handle {
        pub fn new(handle: RawWindowHandle) -> Self {
            let inner = match handle {
                RawWindowHandle::AppKit(handle) => NSWindow::from_raw(handle),
                _ => None,
            };

            Self { inner }
        }

        pub fn get(&self) -> Option<Inner> {
            self.inner.as_deref().map(Message::retain)
        }
    }
}

#[cfg(target_os = "windows")]
mod win32 {
    use raw_window_handle::RawWindowHandle;
    use wfd::HWND;

    pub type Inner = HWND;

    #[derive(Debug, Default)]
    pub struct Handle {
        inner: Option<Inner>,
    }

    impl Clone for Handle {
        fn clone(&self) -> Self {
            Self { inner: self.get() }
        }
    }

    impl Handle {
        pub fn new(handle: RawWindowHandle) -> Self {
            let inner = match handle {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as _),
                _ => None,
            };

            Self { inner }
        }

        pub fn get(&self) -> Option<Inner> {
            self.inner
        }
    }
}

#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android")
))]
mod x11 {
    use raw_window_handle::RawWindowHandle;

    pub type Inner = u64;

    #[derive(Debug, Default)]
    pub struct Handle {
        inner: Option<Inner>,
    }

    impl Clone for Handle {
        fn clone(&self) -> Self {
            Self { inner: self.get() }
        }
    }

    impl Handle {
        pub fn new(handle: RawWindowHandle) -> Self {
            let inner = match handle {
                RawWindowHandle::Xlib(handle) => Some(handle.window as _),
                RawWindowHandle::Xcb(handle) => Some(handle.window.get() as _),
                _ => None,
            };

            Self { inner }
        }

        pub fn get(&self) -> Option<Inner> {
            self.inner
        }
    }
}
