use std::mem;
use std::os::windows::prelude::AsRawHandle;
use std::sync::atomic::{self, AtomicBool};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread::{self, JoinHandle};

use parking_lot::Mutex;
use windows::Win32::Foundation::{HANDLE, LPARAM, S_FALSE, WPARAM};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use windows::Win32::System::Com::CoIncrementMTAUsage;
use windows::Win32::System::Threading::{GetCurrentThreadId, GetThreadId};
use windows::Win32::System::WinRT::{
    CreateDispatcherQueueController, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
    RO_INIT_MULTITHREADED, RoInitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, PostThreadMessageW, TranslateMessage,
    WM_QUIT,
};
use windows::core::Result as WindowsResult;
use windows_future::AsyncActionCompletedHandler;

use crate::d3d11::{self, create_d3d_device};
use crate::frame::Frame;
use crate::graphics_capture_api::{self, GraphicsCaptureApi, InternalCaptureControl};
use crate::settings::{Settings, TryIntoCaptureItemWithType};

#[derive(thiserror::Error, Debug)]
pub enum CaptureControlError<E> {
    #[error("Failed to join thread")]
    FailedToJoinThread,
    #[error("Thread handle is taken out of the struct")]
    ThreadHandleIsTaken,
    #[error("Failed to post thread message")]
    FailedToPostThreadMessage,
    #[error("Stopped handler error: {0}")]
    StoppedHandlerError(E),
    #[error("Windows capture error: {0}")]
    GraphicsCaptureApiError(#[from] GraphicsCaptureApiError<E>),
}

/// Used to control the capture session
pub struct CaptureControl<T: GraphicsCaptureApiHandler + Send + 'static, E> {
    thread_handle: Option<JoinHandle<Result<(), GraphicsCaptureApiError<E>>>>,
    halt_handle: Arc<AtomicBool>,
    callback: Arc<Mutex<T>>,
}

impl<T: GraphicsCaptureApiHandler + Send + 'static, E> CaptureControl<T, E> {
    /// Creates a new Capture Control struct.
    ///
    /// # Arguments
    ///
    /// * `thread_handle` - The join handle for the capture thread.
    /// * `halt_handle` - The atomic boolean used to pause the capture thread.
    /// * `callback` - The mutex-protected callback struct used to call struct
    ///   methods directly.
    ///
    /// # Returns
    ///
    /// The newly created `CaptureControl` struct.
    #[must_use]
    #[inline]
    pub const fn new(
        thread_handle: JoinHandle<Result<(), GraphicsCaptureApiError<E>>>,
        halt_handle: Arc<AtomicBool>,
        callback: Arc<Mutex<T>>,
    ) -> Self {
        Self { thread_handle: Some(thread_handle), halt_handle, callback }
    }

    /// Checks to see if the capture thread is finished.
    ///
    /// # Returns
    ///
    /// `true` if the capture thread is finished, `false` otherwise.
    #[must_use]
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.thread_handle.as_ref().is_none_or(std::thread::JoinHandle::is_finished)
    }

    /// Gets the join handle for the capture thread.
    ///
    /// # Returns
    ///
    /// The join handle for the capture thread.
    #[must_use]
    #[inline]
    pub fn into_thread_handle(self) -> JoinHandle<Result<(), GraphicsCaptureApiError<E>>> {
        self.thread_handle.unwrap()
    }

    /// Gets the halt handle used to pause the capture thread.
    ///
    /// # Returns
    ///
    /// The halt handle used to pause the capture thread.
    #[must_use]
    #[inline]
    pub fn halt_handle(&self) -> Arc<AtomicBool> {
        self.halt_handle.clone()
    }

    /// Gets the callback struct used to call struct methods directly.
    ///
    /// # Returns
    ///
    /// The callback struct used to call struct methods directly.
    #[must_use]
    #[inline]
    pub fn callback(&self) -> Arc<Mutex<T>> {
        self.callback.clone()
    }

    /// Waits for the capturing thread to stop.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the capturing thread stops successfully, an error otherwise.
    #[inline]
    pub fn wait(mut self) -> Result<(), CaptureControlError<E>> {
        if let Some(thread_handle) = self.thread_handle.take() {
            match thread_handle.join() {
                Ok(result) => result?,
                Err(_) => {
                    return Err(CaptureControlError::FailedToJoinThread);
                }
            }
        } else {
            return Err(CaptureControlError::ThreadHandleIsTaken);
        }

        Ok(())
    }

    /// Gracefully stops the capture thread.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the capture thread stops successfully, an error otherwise.
    #[inline]
    pub fn stop(mut self) -> Result<(), CaptureControlError<E>> {
        self.halt_handle.store(true, atomic::Ordering::Relaxed);

        if let Some(thread_handle) = self.thread_handle.take() {
            let handle = thread_handle.as_raw_handle();
            let handle = HANDLE(handle);
            let thread_id = unsafe { GetThreadId(handle) };

            loop {
                match unsafe {
                    PostThreadMessageW(thread_id, WM_QUIT, WPARAM::default(), LPARAM::default())
                } {
                    Ok(()) => break,
                    Err(e) => {
                        if thread_handle.is_finished() {
                            break;
                        }

                        if e.code().0 != -2_147_023_452 {
                            Err(e).map_err(|_| CaptureControlError::FailedToPostThreadMessage)?;
                        }
                    }
                }
            }

            match thread_handle.join() {
                Ok(result) => result?,
                Err(_) => {
                    return Err(CaptureControlError::FailedToJoinThread);
                }
            }
        } else {
            return Err(CaptureControlError::ThreadHandleIsTaken);
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Eq, PartialEq, Clone, Debug)]
pub enum GraphicsCaptureApiError<E> {
    #[error("Failed to join thread")]
    FailedToJoinThread,
    #[error("Failed to initialize WinRT")]
    FailedToInitWinRT,
    #[error("Failed to create dispatcher queue controller")]
    FailedToCreateDispatcherQueueController,
    #[error("Failed to shut down dispatcher queue")]
    FailedToShutdownDispatcherQueue,
    #[error("Failed to set dispatcher queue completed handler")]
    FailedToSetDispatcherQueueCompletedHandler,
    #[error("Failed to convert item to `GraphicsCaptureItem`")]
    ItemConvertFailed,
    #[error("DirectX error: {0}")]
    DirectXError(#[from] d3d11::Error),
    #[error("Graphics capture error: {0}")]
    GraphicsCaptureApiError(graphics_capture_api::Error),
    #[error("New handler error: {0}")]
    NewHandlerError(E),
    #[error("Frame handler error: {0}")]
    FrameHandlerError(E),
}

/// A struct representing the context of the capture handler.
pub struct Context<Flags> {
    /// The flags that are retrieved from the settings.
    pub flags: Flags,
    /// The Direct3D device.
    pub device: ID3D11Device,
    /// The Direct3D device context.
    pub device_context: ID3D11DeviceContext,
}

/// A trait representing a graphics capture handler.
pub trait GraphicsCaptureApiHandler: Sized {
    /// The type of flags used to get the values from the settings.
    type Flags;

    /// The type of error that can occur during capture. The error will be returned from the `CaptureControl` and `start` functions.
    type Error: Send + Sync;

    /// Starts the capture and takes control of the current thread.
    ///
    /// # Arguments
    ///
    /// * `settings` - The capture settings.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the capture was successful; otherwise, it returns an error of type `GraphicsCaptureApiError`.
    #[inline]
    fn start<T: TryIntoCaptureItemWithType>(
        settings: Settings<Self::Flags, T>,
    ) -> Result<(), GraphicsCaptureApiError<Self::Error>>
    where
        Self: Send + 'static,
        <Self as GraphicsCaptureApiHandler>::Flags: Send,
    {
        // Initialize WinRT
        static INIT_MTA: OnceLock<()> = OnceLock::new();
        INIT_MTA.get_or_init(|| {
            unsafe {
                CoIncrementMTAUsage().expect("Failed to increment MTA usage");
            };
        });

        match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(_) => (),
            Err(e) => {
                if e.code() == S_FALSE {
                    // Already initialized
                } else {
                    return Err(GraphicsCaptureApiError::FailedToInitWinRT);
                }
            }
        }

        // Create a dispatcher queue for the current thread
        let options = DispatcherQueueOptions {
            dwSize: u32::try_from(mem::size_of::<DispatcherQueueOptions>()).unwrap(),
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_NONE,
        };
        let controller = unsafe {
            CreateDispatcherQueueController(options)
                .map_err(|_| GraphicsCaptureApiError::FailedToCreateDispatcherQueueController)?
        };

        // Get current thread ID
        let thread_id = unsafe { GetCurrentThreadId() };

        // Create direct3d device and context
        let (d3d_device, d3d_device_context) = create_d3d_device()?;

        // Start capture
        let result = Arc::new(Mutex::new(None));

        let ctx = Context {
            flags: settings.flags,
            device: d3d_device.clone(),
            device_context: d3d_device_context.clone(),
        };

        let callback =
            Arc::new(Mutex::new(Self::new(ctx).map_err(GraphicsCaptureApiError::NewHandlerError)?));

        // Convert the item into a GraphicsCaptureItem and its type
        let (item, item_type) = settings
            .item
            .try_into_capture_item()
            .map_err(|_| GraphicsCaptureApiError::ItemConvertFailed)?;

        let mut capture = GraphicsCaptureApi::new(
            d3d_device,
            d3d_device_context,
            item,
            item_type,
            callback,
            settings.cursor_capture_settings,
            settings.draw_border_settings,
            settings.secondary_window_settings,
            settings.minimum_update_interval_settings,
            settings.dirty_region_settings,
            settings.color_format,
            thread_id,
            result.clone(),
        )
        .map_err(GraphicsCaptureApiError::GraphicsCaptureApiError)?;
        capture.start_capture().map_err(GraphicsCaptureApiError::GraphicsCaptureApiError)?;

        // Message loop
        let mut message = MSG::default();
        unsafe {
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        // Shutdown dispatcher queue
        let async_action = controller
            .ShutdownQueueAsync()
            .map_err(|_| GraphicsCaptureApiError::FailedToShutdownDispatcherQueue)?;

        async_action
            .SetCompleted(&AsyncActionCompletedHandler::new(move |_, _| -> WindowsResult<()> {
                unsafe { PostQuitMessage(0) };
                Ok(())
            }))
            .map_err(|_| GraphicsCaptureApiError::FailedToSetDispatcherQueueCompletedHandler)?;

        // Final message loop
        let mut message = MSG::default();
        unsafe {
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        // Stop capture
        capture.stop_capture();

        // Uninitialize WinRT
        // unsafe { RoUninitialize() }; // Not sure if this is needed here

        // Check handler result
        let result = result.lock().take();
        if let Some(e) = result {
            return Err(GraphicsCaptureApiError::FrameHandlerError(e));
        }

        Ok(())
    }

    /// Starts the capture without taking control of the current thread.
    ///
    /// # Arguments
    ///
    /// * `settings` - The capture settings.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the `CaptureControl` if the capture was successful; otherwise, it returns an error of type `GraphicsCaptureApiError`.
    #[inline]
    fn start_free_threaded<T: TryIntoCaptureItemWithType + Send + 'static>(
        settings: Settings<Self::Flags, T>,
    ) -> Result<CaptureControl<Self, Self::Error>, GraphicsCaptureApiError<Self::Error>>
    where
        Self: Send + 'static,
        <Self as GraphicsCaptureApiHandler>::Flags: Send,
    {
        let (halt_sender, halt_receiver) = mpsc::channel::<Arc<AtomicBool>>();
        let (callback_sender, callback_receiver) = mpsc::channel::<Arc<Mutex<Self>>>();

        let thread_handle =
            thread::spawn(move || -> Result<(), GraphicsCaptureApiError<Self::Error>> {
                // Initialize WinRT
                static INIT_MTA: OnceLock<()> = OnceLock::new();
                INIT_MTA.get_or_init(|| {
                    unsafe {
                        CoIncrementMTAUsage().expect("Failed to increment MTA usage");
                    };
                });

                match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                    Ok(_) => (),
                    Err(e) => {
                        if e.code() == S_FALSE {
                            // Already initialized
                        } else {
                            return Err(GraphicsCaptureApiError::FailedToInitWinRT);
                        }
                    }
                }

                // Create a dispatcher queue for the current thread
                let options = DispatcherQueueOptions {
                    dwSize: u32::try_from(mem::size_of::<DispatcherQueueOptions>()).unwrap(),
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_NONE,
                };
                let controller = unsafe {
                    CreateDispatcherQueueController(options).map_err(|_| {
                        GraphicsCaptureApiError::FailedToCreateDispatcherQueueController
                    })?
                };

                // Get current thread ID
                let thread_id = unsafe { GetCurrentThreadId() };

                // Create direct3d device and context
                let (d3d_device, d3d_device_context) = create_d3d_device()?;

                // Start capture
                let result = Arc::new(Mutex::new(None));

                let ctx = Context {
                    flags: settings.flags,
                    device: d3d_device.clone(),
                    device_context: d3d_device_context.clone(),
                };

                let callback = Arc::new(Mutex::new(
                    Self::new(ctx).map_err(GraphicsCaptureApiError::NewHandlerError)?,
                ));

                // Convert the item into a GraphicsCaptureItem and its type
                let (item, item_type) = settings
                    .item
                    .try_into_capture_item()
                    .map_err(|_| GraphicsCaptureApiError::ItemConvertFailed)?;

                let mut capture = GraphicsCaptureApi::new(
                    d3d_device,
                    d3d_device_context,
                    item,
                    item_type,
                    callback.clone(),
                    settings.cursor_capture_settings,
                    settings.draw_border_settings,
                    settings.secondary_window_settings,
                    settings.minimum_update_interval_settings,
                    settings.dirty_region_settings,
                    settings.color_format,
                    thread_id,
                    result.clone(),
                )
                .map_err(GraphicsCaptureApiError::GraphicsCaptureApiError)?;

                capture
                    .start_capture()
                    .map_err(GraphicsCaptureApiError::GraphicsCaptureApiError)?;

                // Send halt handle
                let halt_handle = capture.halt_handle();
                halt_sender.send(halt_handle).unwrap();

                // Send callback
                callback_sender.send(callback).unwrap();

                // Message loop
                let mut message = MSG::default();
                unsafe {
                    while GetMessageW(&mut message, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }

                // Shutdown dispatcher queue
                let async_action = controller
                    .ShutdownQueueAsync()
                    .map_err(|_| GraphicsCaptureApiError::FailedToShutdownDispatcherQueue)?;

                async_action
                    .SetCompleted(&AsyncActionCompletedHandler::new(
                        move |_, _| -> Result<(), windows::core::Error> {
                            unsafe { PostQuitMessage(0) };
                            Ok(())
                        },
                    ))
                    .map_err(|_| {
                        GraphicsCaptureApiError::FailedToSetDispatcherQueueCompletedHandler
                    })?;

                // Final message loop
                let mut message = MSG::default();
                unsafe {
                    while GetMessageW(&mut message, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }

                // Stop capture
                capture.stop_capture();

                // Uninitialize WinRT
                // unsafe { RoUninitialize() }; // Not sure if this is needed here

                // Check handler result
                let result = result.lock().take();
                if let Some(e) = result {
                    return Err(GraphicsCaptureApiError::FrameHandlerError(e));
                }

                Ok(())
            });

        let Ok(halt_handle) = halt_receiver.recv() else {
            match thread_handle.join() {
                Ok(result) => return Err(result.err().unwrap()),
                Err(_) => {
                    return Err(GraphicsCaptureApiError::FailedToJoinThread);
                }
            }
        };

        let Ok(callback) = callback_receiver.recv() else {
            match thread_handle.join() {
                Ok(result) => return Err(result.err().unwrap()),
                Err(_) => {
                    return Err(GraphicsCaptureApiError::FailedToJoinThread);
                }
            }
        };

        Ok(CaptureControl::new(thread_handle, halt_handle, callback))
    }

    /// Function that will be called to create the struct. The flags can be
    /// passed from settings.
    ///
    /// # Arguments
    ///
    /// * `flags` - The flags used to create the struct.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` if the struct creation was successful; otherwise, it returns an error of type `Self::Error`.
    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error>;

    /// Called every time a new frame is available.
    ///
    /// # Arguments
    ///
    /// * `frame` - A mutable reference to the captured frame.
    /// * `capture_control` - The internal capture control.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the frame processing was successful; otherwise, it returns an error of type `Self::Error`.
    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error>;

    /// Optional handler called when the capture item (usually a window) closes.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the handler executed successfully; otherwise, it returns an error of type `Self::Error`.
    #[inline]
    fn on_closed(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
