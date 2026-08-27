#[cfg(feature = "async-io")]
pub(crate) use async_lock::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
#[cfg(all(feature = "tokio", not(feature = "async-io")))]
pub(crate) use tokio::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// An abstraction over async semaphore API.
#[cfg(feature = "async-io")]
pub(crate) struct Semaphore(async_lock::Semaphore);
#[cfg(all(feature = "tokio", not(feature = "async-io")))]
pub(crate) struct Semaphore(tokio::sync::Semaphore);

impl Semaphore {
    pub const fn new(permits: usize) -> Self {
        #[cfg(feature = "async-io")]
        let semaphore = async_lock::Semaphore::new(permits);
        #[cfg(all(feature = "tokio", not(feature = "async-io")))]
        let semaphore = tokio::sync::Semaphore::const_new(permits);

        Self(semaphore)
    }

    pub async fn acquire(&self) -> SemaphorePermit<'_> {
        #[cfg(feature = "async-io")]
        {
            self.0.acquire().await
        }
        #[cfg(all(feature = "tokio", not(feature = "async-io")))]
        {
            // SAFETY: Since we never explicitly close the sempaphore, `acquire` can't fail.
            self.0.acquire().await.unwrap()
        }
    }
}

#[cfg(feature = "async-io")]
pub(crate) type SemaphorePermit<'a> = async_lock::SemaphoreGuard<'a>;
#[cfg(all(feature = "tokio", not(feature = "async-io")))]
pub(crate) type SemaphorePermit<'a> = tokio::sync::SemaphorePermit<'a>;
