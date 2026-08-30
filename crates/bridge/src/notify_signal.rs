use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct NotifySignal(Arc<NotifySignalInner>);

#[derive(Debug)]
struct NotifySignalInner {
    value: AtomicBool,
    notify: Semaphore,
}

impl NotifySignal {
    pub fn new() -> Self {
        Self(Arc::new(NotifySignalInner {
            value: AtomicBool::new(false),
            notify: Semaphore::new(0),
        }))
    }

    pub fn notify(&self) {
        if !self.0.value.swap(true, Ordering::AcqRel) {
            self.0.notify.add_permits(Semaphore::MAX_PERMITS);
        }
    }

    pub fn is_notified(&self) -> bool {
        self.0.value.load(Ordering::Acquire)
    }

    pub async fn await_notification(&self) {
        if self.is_notified() {
            return;
        }
        let _ = self.0.notify.acquire().await;
    }
}

#[derive(Debug)]
pub struct KeepAliveNotifySignal(NotifySignal);

impl KeepAliveNotifySignal {
    pub fn new() -> Self {
        Self(NotifySignal::new())
    }

    pub fn notify(self) {
        std::mem::drop(self);
    }

    pub fn create_handle(&self) -> KeepAliveNotifySignalHandle {
        KeepAliveNotifySignalHandle(self.0.clone())
    }
}

impl Drop for KeepAliveNotifySignal {
    fn drop(&mut self) {
        self.0.notify();
    }
}

#[derive(Debug, Clone)]
pub struct KeepAliveNotifySignalHandle(NotifySignal);

impl KeepAliveNotifySignalHandle {
    pub async fn await_notification(&self) {
        self.0.await_notification().await
    }

    pub fn is_notified(&self) -> bool {
        self.0.is_notified()
    }

    pub fn is_alive(&self) -> bool {
        !self.is_notified()
    }
}
