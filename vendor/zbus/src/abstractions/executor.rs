#[cfg(feature = "async-io")]
use async_executor::Executor as AsyncExecutor;
#[cfg(feature = "async-io")]
use async_task::Task as AsyncTask;
#[cfg(feature = "tokio")]
use std::io::Error;
#[cfg(not(feature = "async-io"))]
use std::marker::PhantomData;
#[cfg(feature = "async-io")]
use std::sync::Arc;
use std::{
    future::Future,
    io::Result,
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(feature = "tokio")]
use tokio::task::JoinHandle;

/// A wrapper around the underlying runtime/executor.
///
/// This is used to run asynchronous tasks internally and allows integration with various runtimes.
/// See [`crate::Connection::executor`] for an example of integration with external runtimes.
///
/// **Note:** You can (and should) completely ignore this type when the `tokio` backend is in use.
#[derive(Debug, Clone)]
pub struct Executor<'a> {
    #[cfg(feature = "async-io")]
    async_io: Option<Arc<AsyncExecutor<'a>>>,
    #[cfg(not(feature = "async-io"))]
    phantom: PhantomData<&'a ()>,
}

impl Executor<'_> {
    /// Spawns a task onto the executor.
    #[doc(hidden)]
    pub fn spawn<T: Send + 'static>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
        #[allow(unused)] name: &str,
    ) -> Task<T> {
        #[cfg(feature = "async-io")]
        if let Some(executor) = &self.async_io {
            return Task::from_async_io(executor.spawn(future));
        }

        #[cfg(feature = "tokio")]
        return Task::from_tokio(tokio_spawn(future, name));

        #[cfg(all(feature = "async-io", not(feature = "tokio")))]
        unreachable!("async-io executor is always `Some` when tokio is disabled")
    }

    /// Return `true` if there are no unfinished tasks.
    ///
    /// With the `tokio` backend in use, this always returns `true`.
    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "async-io")]
        if let Some(executor) = &self.async_io {
            return executor.is_empty();
        }

        true
    }

    /// Runs a single task.
    ///
    /// With the `tokio` backend in use, it's a noop and never returns.
    pub async fn tick(&self) {
        #[cfg(feature = "async-io")]
        if let Some(executor) = &self.async_io {
            executor.tick().await;
            // Skip the `tokio` branch below (only present when both backends are compiled in).
            #[cfg(feature = "tokio")]
            return;
        }

        #[cfg(feature = "tokio")]
        std::future::pending::<()>().await;
    }

    /// Create a new `Executor`.
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(feature = "async-io")]
            async_io: (!super::use_tokio()).then(|| Arc::new(AsyncExecutor::new())),
            #[cfg(not(feature = "async-io"))]
            phantom: PhantomData,
        }
    }

    /// Whether this executor needs an external driver thread (only the `async-io` backend does).
    #[cfg(feature = "async-io")]
    pub(crate) fn needs_internal_driver(&self) -> bool {
        self.async_io.is_some()
    }

    /// Runs the executor until the given future completes.
    ///
    /// With the `tokio` backend in use, it just awaits on the `future`.
    pub(crate) async fn run<T>(&self, future: impl Future<Output = T>) -> T {
        #[cfg(feature = "async-io")]
        if let Some(executor) = &self.async_io {
            return executor.run(future).await;
        }

        future.await
    }
}

#[cfg(feature = "tokio")]
fn tokio_spawn<T: Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
    #[allow(unused)] name: &str,
) -> JoinHandle<T> {
    #[cfg(tokio_unstable)]
    {
        tokio::task::Builder::new()
            .name(name)
            .spawn(future)
            // SAFETY: Looking at the code, this call always returns an `Ok`.
            .unwrap()
    }
    #[cfg(not(tokio_unstable))]
    {
        tokio::task::spawn(future)
    }
}

/// A wrapper around the task API of the underlying runtime/executor.
///
/// This follows the semantics of `async_task::Task` on drop:
///
/// * it will be cancelled, rather than detached. For detaching, use the `detach` method.
/// * errors from the task cancellation will will be ignored. If you need to know about task errors,
///   convert the task to a `FallibleTask` using the `fallible` method.
#[doc(hidden)]
#[derive(Debug)]
pub struct Task<T> {
    #[cfg(feature = "async-io")]
    async_io: Option<AsyncTask<T>>,
    #[cfg(feature = "tokio")]
    tokio: Option<JoinHandle<T>>,
}

impl<T> Task<T> {
    #[cfg(feature = "async-io")]
    fn from_async_io(task: AsyncTask<T>) -> Self {
        Self {
            async_io: Some(task),
            #[cfg(feature = "tokio")]
            tokio: None,
        }
    }

    #[cfg(feature = "tokio")]
    fn from_tokio(handle: JoinHandle<T>) -> Self {
        Self {
            #[cfg(feature = "async-io")]
            async_io: None,
            tokio: Some(handle),
        }
    }

    /// Detaches the task to let it keep running in the background.
    #[allow(unused_mut)]
    pub fn detach(mut self) {
        #[cfg(feature = "async-io")]
        if let Some(task) = self.async_io.take() {
            task.detach();
        }

        #[cfg(feature = "tokio")]
        if let Some(handle) = self.tokio.take() {
            // Dropping a tokio `JoinHandle` detaches it.
            drop(handle);
        }
    }
}

impl<T> Task<T>
where
    T: Send + 'static,
{
    /// Launch the given blocking function in a task.
    ///
    /// `blocking::unblock` needs no runtime, so async-io's pool is used unless a tokio runtime is
    /// active.
    #[allow(unused)]
    pub(crate) fn spawn_blocking<F>(f: F, #[allow(unused)] name: &str) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        super::select_runtime! {
            tokio: Self::from_tokio(tokio_spawn_blocking(f, name)),
            async_io: Self::from_async_io(blocking::unblock(f)),
        }
    }
}

#[cfg(feature = "tokio")]
fn tokio_spawn_blocking<F, T>(f: F, #[allow(unused)] name: &str) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(tokio_unstable)]
    {
        tokio::task::Builder::new()
            .name(name)
            .spawn_blocking(f)
            // SAFETY: Looking at the code, this call always returns an `Ok`.
            .unwrap()
    }
    #[cfg(not(tokio_unstable))]
    {
        tokio::task::spawn_blocking(f)
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        #[cfg(feature = "tokio")]
        if let Some(join_handle) = self.tokio.take() {
            join_handle.abort();
        }
    }
}

impl<T> Future for Task<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        #[cfg(feature = "async-io")]
        if let Some(task) = &mut this.async_io {
            return Pin::new(task).poll(cx).map(Ok);
        }

        #[cfg(feature = "tokio")]
        if let Some(handle) = &mut this.tokio {
            return Pin::new(handle).poll(cx).map(|r| match r {
                Ok(v) => Ok(v),
                Err(e) => {
                    if e.is_cancelled() {
                        Err(Error::other("tokio::task cancelled"))
                    } else {
                        panic!("tokio::task::JoinHandle error: {e}")
                    }
                }
            });
        }

        unreachable!("Task always has exactly one backend")
    }
}
