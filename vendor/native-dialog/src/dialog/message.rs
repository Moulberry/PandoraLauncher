use super::Dialog;
use crate::ffi::UnsafeWindowHandle;

/// The level of the message in the dialog, which usually affects the color or icon in the dialog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MessageLevel {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug)]
pub struct MessageAlert {
    pub title: String,
    pub text: String,
    pub level: MessageLevel,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for MessageAlert {
    type Output = ();
}

impl MessageAlert {
    super::dialog_delegate!();
}

#[derive(Debug)]
pub struct MessageConfirm {
    pub title: String,
    pub text: String,
    pub level: MessageLevel,
    pub owner: UnsafeWindowHandle,
}

impl Dialog for MessageConfirm {
    type Output = bool;
}

impl MessageConfirm {
    super::dialog_delegate!();
}
