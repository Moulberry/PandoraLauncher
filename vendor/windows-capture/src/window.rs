use std::ptr;

use windows::Graphics::Capture::GraphicsCaptureItem;
use windows::Win32::Foundation::{HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromWindow};
use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetClientRect, GetDesktopWindow,
    GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, WS_CHILD, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, HSTRING, Owned};

use crate::monitor::Monitor;
use crate::settings::{CaptureItemTypes, TryIntoCaptureItemWithType};

#[derive(thiserror::Error, Eq, PartialEq, Clone, Debug)]
pub enum Error {
    #[error("No active window found.")]
    NoActiveWindow,
    #[error("Failed to find a window with the name: {0}")]
    NotFound(String),
    #[error("Failed to convert a Windows string from UTF-16")]
    FailedToConvertWindowsString,
    #[error("A Windows API call failed: {0}")]
    WindowsError(#[from] windows::core::Error),
}

/// Represents a window that can be captured.
///
/// # Example
/// ```no_run
/// use windows_capture::window::Window;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let window = Window::foreground()?;
///     println!("Foreground window title: {}", window.title()?);
///
///     Ok(())
/// }
/// ```
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct Window {
    window: HWND,
}

unsafe impl Send for Window {}

impl Window {
    /// Returns the window that is currently in the foreground.
    ///
    /// # Errors
    ///
    /// Returns `Error::NoActiveWindow` if there is no foreground window.
    #[inline]
    pub fn foreground() -> Result<Self, Error> {
        let window = unsafe { GetForegroundWindow() };

        if window.is_invalid() {
            return Err(Error::NoActiveWindow);
        }

        Ok(Self { window })
    }

    /// Finds a window by its exact title.
    ///
    /// # Arguments
    ///
    /// * `title` - The title of the window to find.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if no window with the specified title is found.
    #[inline]
    pub fn from_name(title: &str) -> Result<Self, Error> {
        let hstring_title = HSTRING::from(title);
        let window = unsafe { FindWindowW(None, &hstring_title)? };

        if window.is_invalid() {
            return Err(Error::NotFound(String::from(title)));
        }

        Ok(Self { window })
    }

    /// Finds a window whose title contains the given substring.
    ///
    /// # Arguments
    ///
    /// * `title` - The substring to search for in window titles.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if no window title contains the specified substring.
    #[inline]
    pub fn from_contains_name(title: &str) -> Result<Self, Error> {
        let windows = Self::enumerate()?;

        let mut target_window = None;
        for window in windows {
            if window.title()?.contains(title) {
                target_window = Some(window);
                break;
            }
        }

        target_window.map_or_else(|| Err(Error::NotFound(String::from(title))), Ok)
    }

    /// Returns the title of the window.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the window title cannot be retrieved.
    #[inline]
    pub fn title(&self) -> Result<String, Error> {
        let len = unsafe { GetWindowTextLengthW(self.window) };

        let mut name = vec![0u16; usize::try_from(len).unwrap() + 1];
        if len >= 1 {
            let copied = unsafe { GetWindowTextW(self.window, &mut name) };
            if copied == 0 {
                return Ok(String::new());
            }
        }

        let name = String::from_utf16(
            &name.as_slice().iter().take_while(|ch| **ch != 0x0000).copied().collect::<Vec<u16>>(),
        )
        .map_err(|_| Error::FailedToConvertWindowsString)?;

        Ok(name)
    }

    /// Returns the process ID of the window.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the process ID cannot be retrieved.
    #[inline]
    pub fn process_id(&self) -> Result<u32, Error> {
        let mut id = 0;
        unsafe { GetWindowThreadProcessId(self.window, Some(&mut id)) };

        if id == 0 {
            return Err(windows::core::Error::from_win32().into());
        }

        Ok(id)
    }

    /// Returns the name of the process that owns the window.
    ///
    /// This function requires the `PROCESS_QUERY_INFORMATION` and `PROCESS_VM_READ` permissions.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the process name cannot be retrieved.
    #[inline]
    pub fn process_name(&self) -> Result<String, Error> {
        let id = self.process_id()?;

        let process =
            unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, id) }?;
        let process = unsafe { Owned::new(process) };

        let mut name = vec![0u16; 260];
        let size = unsafe { GetModuleBaseNameW(*process, None, &mut name) };

        if size == 0 {
            return Err(windows::core::Error::from_win32().into());
        }

        let name = String::from_utf16(
            &name.as_slice().iter().take_while(|ch| **ch != 0x0000).copied().collect::<Vec<u16>>(),
        )
        .map_err(|_| Error::FailedToConvertWindowsString)?;

        Ok(name)
    }

    /// Returns the monitor that has the largest area of intersection with the window.
    ///
    /// Returns `None` if the window does not intersect with any monitor.
    #[must_use]
    #[inline]
    pub fn monitor(&self) -> Option<Monitor> {
        let window = self.window;

        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONULL) };

        if monitor.is_invalid() { None } else { Some(Monitor::from_raw_hmonitor(monitor.0)) }
    }

    /// Returns the bounding rectangle of the window in screen coordinates.
    ///
    /// # Errors
    ///
    /// Returns `Error::WindowsError` if the window rectangle cannot be retrieved.
    #[inline]
    pub fn rect(&self) -> Result<RECT, Error> {
        let mut rect = RECT::default();
        let result = unsafe { GetWindowRect(self.window, &mut rect) };
        if result.is_ok() {
            Ok(rect)
        } else {
            Err(Error::WindowsError(windows::core::Error::from_win32()))
        }
    }

    /// Calculates the height of the window's title bar in pixels.
    ///
    /// # Errors
    ///
    /// Returns `Error` if the title bar height cannot be determined.
    #[inline]
    pub fn title_bar_height(&self) -> Result<u32, Error> {
        let mut window_rect = RECT::default();
        let mut client_rect = RECT::default();

        unsafe {
            DwmGetWindowAttribute(
                self.window,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut window_rect as *mut RECT as *mut std::ffi::c_void,
                std::mem::size_of::<RECT>() as u32,
            )
        }?;

        unsafe { GetClientRect(self.window, &mut client_rect) }?;

        let window_height = window_rect.bottom - window_rect.top;
        let dpi = unsafe { GetDpiForWindow(self.window) };
        let client_height = (client_rect.bottom - client_rect.top) * dpi as i32 / 96;
        let actual_title_height = (window_height - client_height) as i32;

        Ok(actual_title_height as u32)
    }

    /// Checks if the window is a valid target for capture.
    ///
    /// # Returns
    ///
    /// Returns `true` if the window is visible, not a tool window, and not a child window.
    /// Returns `false` otherwise.
    #[must_use]
    #[inline]
    pub fn is_valid(&self) -> bool {
        if !unsafe { IsWindowVisible(self.window).as_bool() } {
            return false;
        }

        let mut id = 0;
        unsafe { GetWindowThreadProcessId(self.window, Some(&mut id)) };
        if id == unsafe { GetCurrentProcessId() } {
            return false;
        }

        let mut rect = RECT::default();
        let result = unsafe { GetClientRect(self.window, &mut rect) };
        if result.is_ok() {
            let styles = unsafe { GetWindowLongPtrW(self.window, GWL_STYLE) };
            let ex_styles = unsafe { GetWindowLongPtrW(self.window, GWL_EXSTYLE) };

            if (ex_styles & isize::try_from(WS_EX_TOOLWINDOW.0).unwrap()) != 0 {
                return false;
            }
            if (styles & isize::try_from(WS_CHILD.0).unwrap()) != 0 {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    /// Returns a list of all capturable windows.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the window enumeration fails.
    #[inline]
    pub fn enumerate() -> Result<Vec<Self>, Error> {
        let mut windows: Vec<Self> = Vec::new();

        unsafe {
            EnumChildWindows(
                Some(GetDesktopWindow()),
                Some(Self::enum_windows_callback),
                LPARAM(ptr::addr_of_mut!(windows) as isize),
            )
            .ok()?;
        };

        Ok(windows)
    }

    /// Creates a `Window` instance from a raw `HWND` handle.
    ///
    /// # Arguments
    ///
    /// * `hwnd` - The raw `HWND` handle.
    #[must_use]
    #[inline]
    pub const fn from_raw_hwnd(hwnd: *mut std::ffi::c_void) -> Self {
        Self { window: HWND(hwnd) }
    }

    /// Returns the raw `HWND` handle of the window.
    #[must_use]
    #[inline]
    pub const fn as_raw_hwnd(&self) -> *mut std::ffi::c_void {
        self.window.0
    }

    // Callback used for enumerating all valid windows.
    #[inline]
    unsafe extern "system" fn enum_windows_callback(window: HWND, vec: LPARAM) -> BOOL {
        let windows = unsafe { &mut *(vec.0 as *mut Vec<Self>) };

        if Self::from_raw_hwnd(window.0).is_valid() {
            windows.push(Self { window });
        }

        TRUE
    }
}

// Implements `TryIntoCaptureItemWithType` for `Window` to convert it to a `GraphicsCaptureItem`.
impl TryIntoCaptureItemWithType for Window {
    #[inline]
    fn try_into_capture_item(
        self,
    ) -> Result<(GraphicsCaptureItem, CaptureItemTypes), windows::core::Error> {
        let window = HWND(self.as_raw_hwnd());

        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item = unsafe { interop.CreateForWindow(window)? };

        Ok((item, CaptureItemTypes::Window(self)))
    }
}
