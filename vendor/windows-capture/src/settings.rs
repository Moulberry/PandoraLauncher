use std::time::Duration;

use windows::Graphics::Capture::GraphicsCaptureItem;

use crate::monitor::Monitor;
use crate::window::Window;

/// An enumeration of the types of items that can be captured.
pub enum CaptureItemTypes {
    /// A display monitor.
    Monitor(Monitor),
    /// An application window.
    Window(Window),
}

/// A trait for types that can be converted into a `GraphicsCaptureItem`.
pub trait TryIntoCaptureItemWithType {
    /// Attempts to convert the object into a `GraphicsCaptureItem` and its corresponding `CaptureItemTypes` variant.
    fn try_into_capture_item(
        self,
    ) -> Result<(GraphicsCaptureItem, CaptureItemTypes), windows::core::Error>;
}

/// Specifies the pixel format for the captured frame.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum ColorFormat {
    /// 16-bit floating-point RGBA format.
    Rgba16F = 10,
    /// 8-bit unsigned integer RGBA format.
    Rgba8 = 28,
    /// 8-bit unsigned integer BGRA format.
    Bgra8 = 87,
}

impl Default for ColorFormat {
    /// The default color format is `Rgba8`.
    #[inline]
    fn default() -> Self {
        Self::Rgba8
    }
}

/// Defines whether the cursor should be visible in the captured output.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum CursorCaptureSettings {
    /// Use the system's default behavior for cursor visibility.
    Default,
    /// Ensure the cursor is always visible in the capture.
    WithCursor,
    /// Ensure the cursor is never visible in the capture.
    WithoutCursor,
}

/// Defines whether a border should be drawn around the captured item.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum DrawBorderSettings {
    /// Use the system's default behavior for the capture border.
    Default,
    /// Draw a border around the captured item.
    WithBorder,
    /// Do not draw a border around the captured item.
    WithoutBorder,
}

/// Defines whether to include or exclude secondary windows in the capture.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum SecondaryWindowSettings {
    /// Use the system's default behavior for capturing secondary windows.
    Default,
    /// Include secondary windows in the capture.
    Include,
    /// Exclude secondary windows from the capture.
    Exclude,
}

/// Defines the minimum interval between frame updates.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum MinimumUpdateIntervalSettings {
    /// Use the system's default update interval.
    Default,
    /// Specify a custom minimum update interval.
    Custom(Duration),
}

/// Defines how the system should handle dirty regions, which are areas of the screen that have changed.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub enum DirtyRegionSettings {
    /// Use the system's default behavior for dirty regions.
    Default,
    /// Only report the dirty regions without rendering them separately.
    ReportOnly,
    /// Report and render the dirty regions.
    ReportAndRender,
}

/// Represents the settings for a screen capture session.
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Settings<Flags, T: TryIntoCaptureItemWithType> {
    /// The item to be captured (e.g., a `Window` or `Monitor`).
    pub(crate) item: T,
    /// Specifies whether the cursor should be captured.
    pub(crate) cursor_capture_settings: CursorCaptureSettings,
    /// Specifies whether a border should be drawn around the captured item.
    pub(crate) draw_border_settings: DrawBorderSettings,
    /// Specifies whether to include secondary windows in the capture.
    pub(crate) secondary_window_settings: SecondaryWindowSettings,
    /// Specifies the minimum time between frame updates.
    pub(crate) minimum_update_interval_settings: MinimumUpdateIntervalSettings,
    /// Specifies how to handle dirty regions.
    pub(crate) dirty_region_settings: DirtyRegionSettings,
    /// The pixel format for the captured frames.
    pub(crate) color_format: ColorFormat,
    /// User-defined flags that can be passed to the capture implementation.
    pub(crate) flags: Flags,
}

impl<Flags, T: TryIntoCaptureItemWithType> Settings<Flags, T> {
    /// Creates a new `Settings` configuration.
    ///
    /// # Arguments
    ///
    /// * `item` - The item to be captured (a `Window` or `Monitor`).
    /// * `cursor_capture_settings` - The desired cursor capture behavior.
    /// * `draw_border_settings` - The desired border drawing behavior.
    /// * `secondary_window_settings` - The desired behavior for capturing secondary windows.
    /// * `minimum_update_interval_settings` - The desired minimum time between frame updates.
    /// * `dirty_region_settings` - The desired behavior for handling dirty regions.
    /// * `color_format` - The desired pixel format for the captured frames.
    /// * `flags` - Custom flags to be passed to the capture implementation's `new` function.
    #[must_use]
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        item: T,
        cursor_capture_settings: CursorCaptureSettings,
        draw_border_settings: DrawBorderSettings,
        secondary_window_settings: SecondaryWindowSettings,
        minimum_update_interval_settings: MinimumUpdateIntervalSettings,
        dirty_region_settings: DirtyRegionSettings,
        color_format: ColorFormat,
        flags: Flags,
    ) -> Self {
        Self {
            item,
            cursor_capture_settings,
            draw_border_settings,
            secondary_window_settings,
            minimum_update_interval_settings,
            dirty_region_settings,
            color_format,
            flags,
        }
    }

    /// Returns a reference to the capture item.
    ///
    /// # Returns
    ///
    /// A reference to the item to be captured.
    #[must_use]
    #[inline]
    pub const fn item(&self) -> &T {
        &self.item
    }

    /// Returns the cursor capture settings.
    ///
    /// # Returns
    ///
    /// The current `CursorCaptureSettings`.
    #[must_use]
    #[inline]
    pub const fn cursor_capture(&self) -> CursorCaptureSettings {
        self.cursor_capture_settings
    }

    /// Returns the draw border settings.
    ///
    /// # Returns
    ///
    /// The current `DrawBorderSettings`.
    #[must_use]
    #[inline]
    pub const fn draw_border(&self) -> DrawBorderSettings {
        self.draw_border_settings
    }

    /// Returns the color format.
    ///
    /// # Returns
    ///
    /// The current `ColorFormat`.
    #[must_use]
    #[inline]
    pub const fn color_format(&self) -> ColorFormat {
        self.color_format
    }

    /// Returns a reference to the flags.
    ///
    /// # Returns
    ///
    /// A reference to the user-defined flags.
    #[must_use]
    #[inline]
    pub const fn flags(&self) -> &Flags {
        &self.flags
    }
}
