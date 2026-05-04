#[cfg(target_os = "macos")]
pub mod mac;

#[cfg(target_os = "windows")]
pub mod win;

mod window_handle;
pub use window_handle::*;
