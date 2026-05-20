use std::sync::Arc;
use std::sync::atomic::{self, AtomicBool};

use parking_lot::Mutex;
use windows::Foundation::Metadata::ApiInformation;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureDirtyRegionMode, GraphicsCaptureItem,
    GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_TEXTURE2D_DESC, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
use windows::core::{HSTRING, IInspectable, Interface};

use crate::capture::GraphicsCaptureApiHandler;
use crate::d3d11::{self, SendDirectX, create_direct3d_device};
use crate::frame::Frame;
use crate::settings::{
    CaptureItemTypes, ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings,
};

#[derive(thiserror::Error, Eq, PartialEq, Clone, Debug)]
pub enum Error {
    #[error("The Graphics Capture API is not supported on this platform.")]
    Unsupported,
    #[error(
        "Toggling cursor capture is not supported by the Graphics Capture API on this platform."
    )]
    CursorConfigUnsupported,
    #[error(
        "Toggling the capture border is not supported by the Graphics Capture API on this platform."
    )]
    BorderConfigUnsupported,
    #[error(
        "Capturing secondary windows is not supported by the Graphics Capture API on this platform."
    )]
    SecondaryWindowsUnsupported,
    #[error(
        "Setting a minimum update interval is not supported by the Graphics Capture API on this platform."
    )]
    MinimumUpdateIntervalUnsupported,
    #[error("Dirty region tracking is not supported by the Graphics Capture API on this platform.")]
    DirtyRegionUnsupported,
    #[error("The capture has already been started.")]
    AlreadyStarted,
    #[error("DirectX error: {0}")]
    DirectXError(#[from] d3d11::Error),
    #[error("Window error: {0}")]
    WindowError(#[from] crate::window::Error),
    #[error("Windows API error: {0}")]
    WindowsError(#[from] windows::core::Error),
}

/// Provides a way to gracefully stop the capture session thread.
pub struct InternalCaptureControl {
    stop: Arc<AtomicBool>,
}

impl InternalCaptureControl {
    /// Creates a new `InternalCaptureControl` struct.
    ///
    /// # Arguments
    ///
    /// * `stop` - An `Arc<AtomicBool>` used to signal the capture thread to stop.
    ///
    /// # Returns
    ///
    /// Returns a new `InternalCaptureControl` instance.
    #[must_use]
    #[inline]
    pub const fn new(stop: Arc<AtomicBool>) -> Self {
        Self { stop }
    }

    /// Signals the capture thread to stop.
    #[inline]
    pub fn stop(self) {
        self.stop.store(true, atomic::Ordering::Relaxed);
    }
}

/// Manages a graphics capture session using the Windows Graphics Capture API.
pub struct GraphicsCaptureApi {
    /// The `GraphicsCaptureItem` to be captured (e.g., a window or monitor).
    item: GraphicsCaptureItem,
    /// The Direct3D 11 device used for the capture.
    _d3d_device: ID3D11Device,
    /// The WinRT `IDirect3DDevice` wrapper.
    _direct3d_device: IDirect3DDevice,
    /// The Direct3D 11 device context.
    _d3d_device_context: ID3D11DeviceContext,
    /// The frame pool that provides frames for the capture session.
    frame_pool: Option<Arc<Direct3D11CaptureFramePool>>,
    /// The graphics capture session itself.
    session: Option<GraphicsCaptureSession>,
    /// An atomic boolean flag to signal the capture thread to stop.
    halt: Arc<AtomicBool>,
    /// A flag indicating whether the capture session is currently active.
    active: bool,
    /// The token for the `Closed` event handler.
    capture_closed_event_token: i64,
    /// The token for the `FrameArrived` event handler.
    frame_arrived_event_token: i64,
}

impl GraphicsCaptureApi {
    /// Creates a new `GraphicsCaptureApi` instance.
    ///
    /// # Arguments
    ///
    /// * `d3d_device` - The `ID3D11Device` to be used for the capture.
    /// * `d3d_device_context` - The `ID3D11DeviceContext` to be used for the capture.
    /// * `item` - The `GraphicsCaptureItem` to be captured.
    /// * `item_type` - The type of the item being captured (e.g., window or monitor).
    /// * `callback` - The user-provided handler for processing captured frames and events.
    /// * `cursor_capture_settings` - The settings for cursor visibility in the capture.
    /// * `draw_border_settings` - The settings for drawing a border around the captured item.
    /// * `secondary_window_settings` - The settings for including secondary windows in the capture.
    /// * `minimum_update_interval_settings` - The settings for the minimum time between frame updates.
    /// * `dirty_region_settings` - The settings for how dirty regions are handled.
    /// * `color_format` - The desired pixel format for the captured frames.
    /// * `thread_id` - The ID of the thread that owns the message loop.
    /// * `result` - An `Arc<Mutex<Option<E>>>` to store any errors that occur in the callbacks.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the new `GraphicsCaptureApi` instance if successful,
    /// or an `Error` if initialization fails.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn new<
        T: GraphicsCaptureApiHandler<Error = E> + Send + 'static,
        E: Send + Sync + 'static,
    >(
        d3d_device: ID3D11Device,
        d3d_device_context: ID3D11DeviceContext,
        item: GraphicsCaptureItem,
        item_type: CaptureItemTypes,
        callback: Arc<Mutex<T>>,
        cursor_capture_settings: CursorCaptureSettings,
        draw_border_settings: DrawBorderSettings,
        secondary_window_settings: SecondaryWindowSettings,
        minimum_update_interval_settings: MinimumUpdateIntervalSettings,
        dirty_region_settings: DirtyRegionSettings,
        color_format: ColorFormat,
        thread_id: u32,
        result: Arc<Mutex<Option<E>>>,
    ) -> Result<Self, Error> {
        // Check support
        if !Self::is_supported()? {
            return Err(Error::Unsupported);
        }

        if cursor_capture_settings != CursorCaptureSettings::Default
            && !Self::is_cursor_settings_supported()?
        {
            return Err(Error::CursorConfigUnsupported);
        }

        if draw_border_settings != DrawBorderSettings::Default
            && !Self::is_border_settings_supported()?
        {
            return Err(Error::BorderConfigUnsupported);
        }

        if secondary_window_settings != SecondaryWindowSettings::Default
            && !Self::is_secondary_windows_supported()?
        {
            return Err(Error::SecondaryWindowsUnsupported);
        }

        if minimum_update_interval_settings != MinimumUpdateIntervalSettings::Default
            && !Self::is_minimum_update_interval_supported()?
        {
            return Err(Error::MinimumUpdateIntervalUnsupported);
        }

        if dirty_region_settings != DirtyRegionSettings::Default
            && !Self::is_dirty_region_supported()?
        {
            return Err(Error::DirtyRegionUnsupported);
        }

        // Pre-calculate the title bar height so each frame doesn't need to do it
        let title_bar_height = match item_type {
            CaptureItemTypes::Window(window) => Some(window.title_bar_height()?),
            CaptureItemTypes::Monitor(_) => None,
        };

        // Create DirectX devices
        let direct3d_device = create_direct3d_device(&d3d_device)?;

        let pixel_format = DirectXPixelFormat(color_format as i32);

        // Create frame pool
        let frame_pool =
            Direct3D11CaptureFramePool::Create(&direct3d_device, pixel_format, 1, item.Size()?)?;
        let frame_pool = Arc::new(frame_pool);

        // Create capture session
        let session = frame_pool.CreateCaptureSession(&item)?;

        // Preallocate a buffer for frame data to avoid reallocations.
        // The size is based on a 4K display (3840x2160) with 4 bytes per pixel (RGBA).
        let mut buffer = vec![0u8; 3840 * 2160 * 4];

        // Indicates if the capture is closed
        let halt = Arc::new(AtomicBool::new(false));

        // Set capture session closed event
        let capture_closed_event_token = item.Closed(&TypedEventHandler::<
            GraphicsCaptureItem,
            IInspectable,
        >::new({
            // Init
            let callback_closed = callback.clone();
            let halt_closed = halt.clone();
            let result_closed = result.clone();

            move |_, _| {
                halt_closed.store(true, atomic::Ordering::Relaxed);

                // Notify the user that the capture session is closed.
                let callback_closed = callback_closed.lock().on_closed();
                if let Err(e) = callback_closed {
                    *result_closed.lock() = Some(e);
                }

                // Stop the message loop to allow the thread to exit gracefully.
                unsafe {
                    PostThreadMessageW(thread_id, WM_QUIT, WPARAM::default(), LPARAM::default())?;
                };

                Result::Ok(())
            }
        }))?;

        // Set frame pool frame arrived event
        let frame_arrived_event_token = frame_pool.FrameArrived(&TypedEventHandler::<
            Direct3D11CaptureFramePool,
            IInspectable,
        >::new({
            // Init
            let frame_pool_recreate = frame_pool.clone();
            let halt_frame_pool = halt.clone();
            let d3d_device_frame_pool = d3d_device.clone();
            let context = d3d_device_context.clone();
            let result_frame_pool = result;

            let mut last_size = item.Size()?;
            let callback_frame_pool = callback;
            let direct3d_device_recreate = SendDirectX::new(direct3d_device.clone());

            move |frame, _| {
                // Return early if the capture is closed
                if halt_frame_pool.load(atomic::Ordering::Relaxed) {
                    return Ok(());
                }

                // Get frame
                let frame = frame
                    .as_ref()
                    .expect("FrameArrived parameter was None this should never happen.")
                    .TryGetNextFrame()?;
                let timestamp = frame.SystemRelativeTime()?;

                // Get frame content size
                let frame_content_size = frame.ContentSize()?;

                // Get frame surface
                let frame_surface = frame.Surface()?;

                // Convert surface to texture
                let frame_dxgi_interface = frame_surface.cast::<IDirect3DDxgiInterfaceAccess>()?;
                let frame_texture =
                    unsafe { frame_dxgi_interface.GetInterface::<ID3D11Texture2D>()? };

                // Get texture settings
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                unsafe { frame_texture.GetDesc(&mut desc) }

                // Check if the size has been changed
                if frame_content_size.Width != last_size.Width
                    || frame_content_size.Height != last_size.Height
                {
                    let direct3d_device_recreate = &direct3d_device_recreate;
                    frame_pool_recreate.Recreate(
                        &direct3d_device_recreate.0,
                        pixel_format,
                        1,
                        frame_content_size,
                    )?;

                    last_size = frame_content_size;

                    return Ok(());
                }

                // Set width & height
                let texture_width = desc.Width;
                let texture_height = desc.Height;

                // Create a frame
                let mut frame = Frame::new(
                    &d3d_device_frame_pool,
                    frame_surface,
                    frame_texture,
                    timestamp,
                    &context,
                    &mut buffer,
                    texture_width,
                    texture_height,
                    color_format,
                    title_bar_height,
                );

                // Init internal capture control
                let stop = Arc::new(AtomicBool::new(false));
                let internal_capture_control = InternalCaptureControl::new(stop.clone());

                // Send the frame to the callback struct
                let result = callback_frame_pool
                    .lock()
                    .on_frame_arrived(&mut frame, internal_capture_control);

                // If the user signals to stop or an error occurs, halt the capture.
                if stop.load(atomic::Ordering::Relaxed) || result.is_err() {
                    if let Err(e) = result {
                        *result_frame_pool.lock() = Some(e);
                    }

                    halt_frame_pool.store(true, atomic::Ordering::Relaxed);

                    // Stop the message loop to allow the thread to exit gracefully.
                    unsafe {
                        PostThreadMessageW(
                            thread_id,
                            WM_QUIT,
                            WPARAM::default(),
                            LPARAM::default(),
                        )?;
                    };
                }

                Result::Ok(())
            }
        }))?;

        if cursor_capture_settings != CursorCaptureSettings::Default {
            if Self::is_cursor_settings_supported()? {
                match cursor_capture_settings {
                    CursorCaptureSettings::Default => (),
                    CursorCaptureSettings::WithCursor => session.SetIsCursorCaptureEnabled(true)?,
                    CursorCaptureSettings::WithoutCursor => {
                        session.SetIsCursorCaptureEnabled(false)?
                    }
                };
            } else {
                return Err(Error::CursorConfigUnsupported);
            }
        }

        if draw_border_settings != DrawBorderSettings::Default {
            if Self::is_border_settings_supported()? {
                match draw_border_settings {
                    DrawBorderSettings::Default => (),
                    DrawBorderSettings::WithBorder => {
                        session.SetIsBorderRequired(true)?;
                    }
                    DrawBorderSettings::WithoutBorder => session.SetIsBorderRequired(false)?,
                }
            } else {
                return Err(Error::BorderConfigUnsupported);
            }
        }

        if secondary_window_settings != SecondaryWindowSettings::Default {
            if Self::is_secondary_windows_supported()? {
                match secondary_window_settings {
                    SecondaryWindowSettings::Default => (),
                    SecondaryWindowSettings::Include => session.SetIncludeSecondaryWindows(true)?,
                    SecondaryWindowSettings::Exclude => {
                        session.SetIncludeSecondaryWindows(false)?
                    }
                }
            } else {
                return Err(Error::SecondaryWindowsUnsupported);
            }
        }

        if minimum_update_interval_settings != MinimumUpdateIntervalSettings::Default {
            if Self::is_minimum_update_interval_supported()? {
                match minimum_update_interval_settings {
                    MinimumUpdateIntervalSettings::Default => (),
                    MinimumUpdateIntervalSettings::Custom(duration) => {
                        session.SetMinUpdateInterval(duration.into())?;
                    }
                }
            } else {
                return Err(Error::MinimumUpdateIntervalUnsupported);
            }
        }

        if dirty_region_settings != DirtyRegionSettings::Default {
            if Self::is_dirty_region_supported()? {
                match dirty_region_settings {
                    DirtyRegionSettings::Default => (),
                    DirtyRegionSettings::ReportOnly => {
                        session.SetDirtyRegionMode(GraphicsCaptureDirtyRegionMode::ReportOnly)?
                    }
                    DirtyRegionSettings::ReportAndRender => session
                        .SetDirtyRegionMode(GraphicsCaptureDirtyRegionMode::ReportAndRender)?,
                }
            } else {
                return Err(Error::DirtyRegionUnsupported);
            }
        }

        Ok(Self {
            item,
            _d3d_device: d3d_device,
            _direct3d_device: direct3d_device,
            _d3d_device_context: d3d_device_context,
            frame_pool: Some(frame_pool),
            session: Some(session),
            halt,
            active: false,
            frame_arrived_event_token,
            capture_closed_event_token,
        })
    }

    /// Start the capture.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the capture started successfully, or an `Error` if
    /// an error occurred.
    #[inline]
    pub fn start_capture(&mut self) -> Result<(), Error> {
        if self.active {
            return Err(Error::AlreadyStarted);
        }

        self.session.as_ref().unwrap().StartCapture()?;
        self.active = true;

        Ok(())
    }

    /// Stops the capture session and cleans up resources.
    #[inline]
    pub fn stop_capture(mut self) {
        if let Some(frame_pool) = self.frame_pool.take() {
            frame_pool
                .RemoveFrameArrived(self.frame_arrived_event_token)
                .expect("Failed to remove Frame Arrived event handler");

            frame_pool.Close().expect("Failed to Close Frame Pool");
        }

        if let Some(session) = self.session.take() {
            session.Close().expect("Failed to Close Capture Session");
        }

        self.item
            .RemoveClosed(self.capture_closed_event_token)
            .expect("Failed to remove Capture Session Closed event handler");
    }

    /// Get the halt handle.
    ///
    /// # Returns
    ///
    /// Returns an `Arc<AtomicBool>` that can be used to check if the capture is halted.
    #[must_use]
    #[inline]
    pub fn halt_handle(&self) -> Arc<AtomicBool> {
        self.halt.clone()
    }

    /// Check if the Windows Graphics Capture API is supported.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the API is supported, `Ok(false)` otherwise, or an `Error` if the check fails.
    #[inline]
    pub fn is_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsApiContractPresentByMajor(
            &HSTRING::from("Windows.Foundation.UniversalApiContract"),
            8,
        )? && GraphicsCaptureSession::IsSupported()?)
    }

    /// Checks if the cursor capture settings can be changed.
    ///
    /// # Returns
    ///
    /// Returns `true` if toggling cursor capture is supported, `false` otherwise.
    #[inline]
    pub fn is_cursor_settings_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsPropertyPresent(
            &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &HSTRING::from("IsCursorCaptureEnabled"),
        )? && Self::is_supported()?)
    }

    /// Checks if the capture border settings can be changed.
    ///
    /// # Returns
    ///
    /// Returns `true` if toggling the capture border is supported, `false` otherwise.
    #[inline]
    pub fn is_border_settings_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsPropertyPresent(
            &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &HSTRING::from("IsBorderRequired"),
        )? && Self::is_supported()?)
    }

    /// Checks if capturing secondary windows is supported.
    ///
    /// # Returns
    ///
    /// Returns `true` if capturing secondary windows is supported, `false` otherwise.
    #[inline]
    pub fn is_secondary_windows_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsPropertyPresent(
            &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &HSTRING::from("IncludeSecondaryWindows"),
        )? && Self::is_supported()?)
    }

    /// Checks if setting a minimum update interval is supported.
    ///
    /// # Returns
    ///
    /// Returns `true` if setting a minimum update interval is supported, `false` otherwise.
    #[inline]
    pub fn is_minimum_update_interval_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsPropertyPresent(
            &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &HSTRING::from("MinUpdateInterval"),
        )? && Self::is_supported()?)
    }

    /// Checks if dirty region tracking is supported.
    ///
    /// # Returns
    ///
    /// Returns `true` if dirty region settings are supported, `false` otherwise.
    #[inline]
    pub fn is_dirty_region_supported() -> Result<bool, Error> {
        Ok(ApiInformation::IsPropertyPresent(
            &HSTRING::from("Windows.Graphics.Capture.GraphicsCaptureSession"),
            &HSTRING::from("DirtyRegionMode"),
        )? && Self::is_supported()?)
    }
}

impl Drop for GraphicsCaptureApi {
    fn drop(&mut self) {
        if let Some(frame_pool) = self.frame_pool.take() {
            let _ = frame_pool.RemoveFrameArrived(self.frame_arrived_event_token);
            let _ = frame_pool.Close();
        }

        if let Some(session) = self.session.take() {
            let _ = session.Close();
        }

        let _ = self.item.RemoveClosed(self.capture_closed_event_token);
    }
}
