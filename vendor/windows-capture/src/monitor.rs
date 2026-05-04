use std::num::ParseIntError;
use std::string::FromUtf16Error;
use std::{mem, ptr};

use windows::Graphics::Capture::GraphicsCaptureItem;
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS, DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY,
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS,
    QueryDisplayConfig,
};
use windows::Win32::Foundation::{LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, DISPLAY_DEVICE_STATE_FLAGS, DISPLAY_DEVICEW, ENUM_CURRENT_SETTINGS,
    EnumDisplayDevicesW, EnumDisplayMonitors, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR,
    MONITOR_DEFAULTTONULL, MONITORINFO, MONITORINFOEXW, MonitorFromPoint,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{BOOL, HSTRING, PCWSTR};

use crate::settings::{CaptureItemTypes, TryIntoCaptureItemWithType};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find the specified monitor.")]
    NotFound,
    #[error("Failed to get the monitor's name.")]
    NameNotFound,
    #[error("The monitor index must be greater than zero.")]
    IndexIsLowerThanOne,
    #[error("Failed to get monitor information.")]
    FailedToGetMonitorInfo,
    #[error("Failed to get the monitor's display settings.")]
    FailedToGetMonitorSettings,
    #[error("Failed to get the monitor's device name.")]
    FailedToGetMonitorName,
    #[error("Failed to parse the monitor index: {0}")]
    FailedToParseMonitorIndex(#[from] ParseIntError),
    #[error("Failed to convert a Windows string: {0}")]
    FailedToConvertWindowsString(#[from] FromUtf16Error),
    #[error("A Windows API call failed: {0}")]
    WindowsError(#[from] windows::core::Error),
}

/// Represents a display monitor.
///
/// # Example
/// ```no_run
/// use windows_capture::monitor::Monitor;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let monitor = Monitor::primary()?;
///     println!("Primary Monitor: {}", monitor.name()?);
///
///     Ok(())
/// }
/// ```
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct Monitor {
    monitor: HMONITOR,
}

unsafe impl Send for Monitor {}

impl Monitor {
    /// Returns the primary monitor.
    ///
    /// # Errors
    ///
    /// Returns `Error::NotFound` if the primary monitor cannot be found.
    #[inline]
    pub fn primary() -> Result<Self, Error> {
        let point = POINT { x: 0, y: 0 };
        let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONULL) };

        if monitor.is_invalid() {
            return Err(Error::NotFound);
        }

        Ok(Self { monitor })
    }

    /// Returns the monitor at the specified index.
    ///
    /// # Arguments
    ///
    /// * `index` - The one-based index of the monitor to retrieve.
    ///
    /// # Errors
    ///
    /// * `Error::IndexIsLowerThanOne` - If the `index` is less than 1.
    /// * `Error::NotFound` - If no monitor is found at the specified `index`.
    #[inline]
    pub fn from_index(index: usize) -> Result<Self, Error> {
        if index < 1 {
            return Err(Error::IndexIsLowerThanOne);
        }

        let monitor = Self::enumerate()?;
        let monitor = match monitor.get(index - 1) {
            Some(monitor) => *monitor,
            None => return Err(Error::NotFound),
        };

        Ok(monitor)
    }

    /// Returns the one-based index of the monitor.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's device name cannot be parsed to determine the index.
    #[inline]
    pub fn index(&self) -> Result<usize, Error> {
        let device_name = self.device_name()?;
        Ok(device_name.replace("\\\\.\\DISPLAY", "").parse()?)
    }

    /// Returns the friendly name of the monitor.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's name cannot be retrieved.
    #[inline]
    pub fn name(&self) -> Result<String, Error> {
        let mut monitor_info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: u32::try_from(mem::size_of::<MONITORINFOEXW>()).unwrap(),
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            },
            szDevice: [0; 32],
        };
        if unsafe {
            !GetMonitorInfoW(
                HMONITOR(self.as_raw_hmonitor()),
                std::ptr::addr_of_mut!(monitor_info).cast(),
            )
            .as_bool()
        } {
            return Err(Error::FailedToGetMonitorInfo);
        }

        let mut number_of_paths = 0;
        let mut number_of_modes = 0;
        unsafe {
            GetDisplayConfigBufferSizes(
                QDC_ONLY_ACTIVE_PATHS,
                &mut number_of_paths,
                &mut number_of_modes,
            )
            .ok()?;
        };

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); number_of_paths as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); number_of_modes as usize];
        unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut number_of_paths,
                paths.as_mut_ptr(),
                &mut number_of_modes,
                modes.as_mut_ptr(),
                None,
            )
        }
        .ok()?;

        for path in paths {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: u32::try_from(mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>())
                        .unwrap(),
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                viewGdiDeviceName: [0; 32],
            };

            let device_name = self.device_name()?;
            let view_gdi_device_name = String::from_utf16(
                &monitor_info
                    .szDevice
                    .as_slice()
                    .iter()
                    .take_while(|ch| **ch != 0x0000)
                    .copied()
                    .collect::<Vec<u16>>(),
            )?;

            if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } == 0
                && device_name == view_gdi_device_name
            {
                let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                    header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                        r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                        size: u32::try_from(mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>())
                            .unwrap(),
                        adapterId: path.sourceInfo.adapterId,
                        id: path.targetInfo.id,
                    },
                    flags: DISPLAYCONFIG_TARGET_DEVICE_NAME_FLAGS::default(),
                    outputTechnology: DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY::default(),
                    edidManufactureId: 0,
                    edidProductCodeId: 0,
                    connectorInstance: 0,
                    monitorFriendlyDeviceName: [0; 64],
                    monitorDevicePath: [0; 128],
                };

                if unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } == 0 {
                    let name = String::from_utf16(
                        &target
                            .monitorFriendlyDeviceName
                            .as_slice()
                            .iter()
                            .take_while(|ch| **ch != 0x0000)
                            .copied()
                            .collect::<Vec<u16>>(),
                    )?;
                    return Ok(name);
                }

                return Err(Error::FailedToGetMonitorInfo);
            }
        }

        Err(Error::NameNotFound)
    }

    /// Returns the device name of the monitor (e.g., `\\.\DISPLAY1`).
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's device name cannot be retrieved.
    #[inline]
    pub fn device_name(&self) -> Result<String, Error> {
        let mut monitor_info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: u32::try_from(mem::size_of::<MONITORINFOEXW>()).unwrap(),
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            },
            szDevice: [0; 32],
        };
        if unsafe {
            !GetMonitorInfoW(
                HMONITOR(self.as_raw_hmonitor()),
                std::ptr::addr_of_mut!(monitor_info).cast(),
            )
            .as_bool()
        } {
            return Err(Error::FailedToGetMonitorInfo);
        }

        let device_name = String::from_utf16(
            &monitor_info
                .szDevice
                .as_slice()
                .iter()
                .take_while(|ch| **ch != 0x0000)
                .copied()
                .collect::<Vec<u16>>(),
        )?;

        Ok(device_name)
    }

    /// Returns the device string of the monitor (e.g., `NVIDIA GeForce RTX 4090`).
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's device string cannot be retrieved.
    #[inline]
    pub fn device_string(&self) -> Result<String, Error> {
        let mut monitor_info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: u32::try_from(mem::size_of::<MONITORINFOEXW>()).unwrap(),
                rcMonitor: RECT::default(),
                rcWork: RECT::default(),
                dwFlags: 0,
            },
            szDevice: [0; 32],
        };
        if unsafe {
            !GetMonitorInfoW(
                HMONITOR(self.as_raw_hmonitor()),
                std::ptr::addr_of_mut!(monitor_info).cast(),
            )
            .as_bool()
        } {
            return Err(Error::FailedToGetMonitorInfo);
        }

        let mut display_device = DISPLAY_DEVICEW {
            cb: u32::try_from(mem::size_of::<DISPLAY_DEVICEW>()).unwrap(),
            DeviceName: [0; 32],
            DeviceString: [0; 128],
            StateFlags: DISPLAY_DEVICE_STATE_FLAGS::default(),
            DeviceID: [0; 128],
            DeviceKey: [0; 128],
        };

        if unsafe {
            !EnumDisplayDevicesW(
                PCWSTR::from_raw(monitor_info.szDevice.as_mut_ptr()),
                0,
                &mut display_device,
                0,
            )
            .as_bool()
        } {
            return Err(Error::FailedToGetMonitorName);
        }

        let device_string = String::from_utf16(
            &display_device
                .DeviceString
                .as_slice()
                .iter()
                .take_while(|ch| **ch != 0x0000)
                .copied()
                .collect::<Vec<u16>>(),
        )?;

        Ok(device_string)
    }

    /// Returns the refresh rate of the monitor in hertz (Hz).
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's refresh rate cannot be retrieved.
    #[inline]
    pub fn refresh_rate(&self) -> Result<u32, Error> {
        let mut device_mode = DEVMODEW {
            dmSize: u16::try_from(mem::size_of::<DEVMODEW>()).unwrap(),
            ..DEVMODEW::default()
        };
        let name = HSTRING::from(self.device_name()?);
        if unsafe {
            !EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut device_mode)
                .as_bool()
        } {
            return Err(Error::FailedToGetMonitorSettings);
        }

        Ok(device_mode.dmDisplayFrequency)
    }

    /// Returns the width of the monitor in pixels.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's width cannot be retrieved.
    #[inline]
    pub fn width(&self) -> Result<u32, Error> {
        let mut device_mode = DEVMODEW {
            dmSize: u16::try_from(mem::size_of::<DEVMODEW>()).unwrap(),
            ..DEVMODEW::default()
        };
        let name = HSTRING::from(self.device_name()?);
        if unsafe {
            !EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut device_mode)
                .as_bool()
        } {
            return Err(Error::FailedToGetMonitorSettings);
        }

        Ok(device_mode.dmPelsWidth)
    }

    /// Returns the height of the monitor in pixels.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor's height cannot be retrieved.
    #[inline]
    pub fn height(&self) -> Result<u32, Error> {
        let mut device_mode = DEVMODEW {
            dmSize: u16::try_from(mem::size_of::<DEVMODEW>()).unwrap(),
            ..DEVMODEW::default()
        };
        let name = HSTRING::from(self.device_name()?);
        if unsafe {
            !EnumDisplaySettingsW(PCWSTR(name.as_ptr()), ENUM_CURRENT_SETTINGS, &mut device_mode)
                .as_bool()
        } {
            return Err(Error::FailedToGetMonitorSettings);
        }

        Ok(device_mode.dmPelsHeight)
    }

    /// Returns a list of all available monitors.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if the monitor enumeration fails.
    #[inline]
    pub fn enumerate() -> Result<Vec<Self>, Error> {
        let mut monitors: Vec<Self> = Vec::new();

        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(Self::enum_monitors_callback),
                LPARAM(ptr::addr_of_mut!(monitors) as isize),
            )
            .ok()?;
        };

        Ok(monitors)
    }

    /// Creates a `Monitor` instance from a raw `HMONITOR` handle.
    ///
    /// # Arguments
    ///
    /// * `hmonitor` - The raw `HMONITOR` handle.
    #[must_use]
    #[inline]
    pub const fn from_raw_hmonitor(monitor: *mut std::ffi::c_void) -> Self {
        Self { monitor: HMONITOR(monitor) }
    }

    /// Returns the raw `HMONITOR` handle of the monitor.
    #[must_use]
    #[inline]
    pub const fn as_raw_hmonitor(&self) -> *mut std::ffi::c_void {
        self.monitor.0
    }

    // Callback used for enumerating all monitors.
    #[inline]
    unsafe extern "system" fn enum_monitors_callback(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        vec: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(vec.0 as *mut Vec<Self>) };

        monitors.push(Self { monitor });

        TRUE
    }
}

// Implements `TryIntoCaptureItemWithType` for `Monitor` to convert it to a `GraphicsCaptureItem`.
impl TryIntoCaptureItemWithType for Monitor {
    #[inline]
    fn try_into_capture_item(
        self,
    ) -> Result<(GraphicsCaptureItem, CaptureItemTypes), windows::core::Error> {
        let monitor = HMONITOR(self.as_raw_hmonitor());

        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;

        let item = unsafe { interop.CreateForMonitor(monitor)? };

        Ok((item, CaptureItemTypes::Monitor(self)))
    }
}
