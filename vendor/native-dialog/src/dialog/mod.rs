pub mod file;
pub use file::*;

pub mod filter;
pub use filter::*;

pub mod message;
pub use message::*;

pub trait Dialog {
    type Output;
}

macro_rules! dialog_delegate {
    () => {
        pub fn show(self) -> $crate::Result<<Self as $crate::dialog::Dialog>::Output> {
            $crate::dialog::DialogImpl::show(self)
        }

        #[cfg(feature = "async")]
        pub async fn spawn(self) -> $crate::Result<<Self as $crate::dialog::Dialog>::Output> {
            $crate::dialog::DialogImpl::spawn(self).await
        }
    };
}

use dialog_delegate;

pub trait DialogImpl: Dialog {
    fn show(self) -> crate::Result<Self::Output>;

    #[cfg(feature = "async")]
    fn spawn(self) -> impl std::future::Future<Output = crate::Result<Self::Output>> + Send;
}

#[cfg(target_os = "macos")]
mod mac;

#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(target_os = "ios"),
    not(target_os = "android")
))]
mod gnu;

#[cfg(target_os = "windows")]
mod win;
