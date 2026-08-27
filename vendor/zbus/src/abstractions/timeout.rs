use crate::{Error, Result};
use std::{future::Future, io::ErrorKind, time::Duration};

#[cfg(feature = "tokio")]
async fn timeout_tokio<F, T>(fut: F, timeout: Duration) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    tokio::time::timeout(timeout, fut).await.map_err(|_| {
        Error::from(std::io::Error::new(
            ErrorKind::TimedOut,
            "timed out".to_string(),
        ))
    })?
}

#[cfg(feature = "async-io")]
async fn timeout_async_io<F, T>(fut: F, timeout: Duration) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    use futures_lite::FutureExt;

    fut.or(async {
        async_io::Timer::after(timeout).await;

        Err(Error::from(std::io::Error::new(
            ErrorKind::TimedOut,
            "timed out",
        )))
    })
    .await
}

/// Awaits a future with a provided timeout.
pub(crate) async fn timeout<F, T>(fut: F, timeout: Duration) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    crate::abstractions::select_runtime! {
        tokio: timeout_tokio(fut, timeout).await,
        async_io: timeout_async_io(fut, timeout).await,
    }
}
