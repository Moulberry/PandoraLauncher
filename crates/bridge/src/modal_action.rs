use std::{
    ops::Deref,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use atomic_time::AtomicOptionInstant;
use tokio_util::sync::CancellationToken;

use crate::{handle::FrontendHandle, message::MessageToFrontend, serial::AtomicOptionSerial};

#[derive(Default, Clone, Debug)]
pub struct ModalAction {
    inner: Arc<ModalActionInner>,
}

impl ModalAction {
    pub fn refcnt(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl Deref for ModalAction {
    type Target = ModalActionInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug)]
pub struct ModalActionVisitUrl {
    pub message: Arc<str>,
    pub url: Arc<str>,
    pub prevent_auto_finish: bool,
}

#[derive(Default)]
pub struct ModalActionInner {
    pub finished_at: AtomicOptionInstant,
    pub error: RwLock<Option<Arc<str>>>,
    pub visit_url: RwLock<Option<ModalActionVisitUrl>>,
    pub trackers: ProgressTrackers,
    pub request_cancel: CancellationToken,
}

impl ModalActionInner {
    pub fn set_finished(&self) {
        let _ = self.finished_at.compare_exchange(None, Some(Instant::now()), Ordering::SeqCst, Ordering::Relaxed);
    }

    pub fn get_finished_at(&self) -> Option<Instant> {
        self.finished_at.load(Ordering::SeqCst)
    }

    pub fn set_error_message(&self, error: Arc<str>) {
        *self.error.write().unwrap() = Some(error);
    }

    pub fn set_visit_url(&self, visit_url: ModalActionVisitUrl) {
        *self.visit_url.write().unwrap() = Some(visit_url);
    }

    pub fn unset_visit_url(&self) {
        *self.visit_url.write().unwrap() = None;
    }

    pub fn request_cancel(&self) {
        self.request_cancel.cancel();
    }

    pub fn has_requested_cancel(&self) -> bool {
        self.request_cancel.is_cancelled()
    }
}

impl std::fmt::Debug for ModalActionInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModalActionInner")
            .field("finished_at", &self.finished_at.load(Ordering::Relaxed))
            .field("error", &self.error)
            .field("visit_url", &self.visit_url)
            .field("trackers", &self.trackers)
            .field("request_cancel", &self.request_cancel)
            .finish()
    }
}

#[derive(Default, Clone, Debug)]
pub struct ProgressTrackers {
    pub trackers: Arc<RwLock<Vec<ProgressTracker>>>,
}

impl ProgressTrackers {
    pub fn push(&self, tracker: ProgressTracker) {
        self.trackers.write().unwrap().push(tracker);
    }

    pub fn clear(&self) {
        self.trackers.write().unwrap().clear();
    }
}

#[derive(Clone, Debug)]
pub struct ProgressTracker {
    inner: Arc<ProgressTrackerInner>,
    sender: FrontendHandle,
    notify_serial: AtomicOptionSerial,
}

struct ProgressTrackerInner {
    count: AtomicUsize,
    total: AtomicUsize,
    finished_at: AtomicOptionInstant,
    finish_type: AtomicProgressTrackerFinishType,
    title: RwLock<Arc<str>>,
}

#[atomic_enum::atomic_enum]
#[derive(PartialEq, Eq)]
pub enum ProgressTrackerFinishType {
    Normal,
    Error,
    Fast,
}

impl ProgressTrackerFinishType {
    pub fn from_err(error: bool) -> Self {
        if error {
            Self::Error
        } else {
            Self::Normal
        }
    }
}

impl std::fmt::Debug for ProgressTrackerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressTrackerInner")
            .field("count", &self.count)
            .field("total", &self.total)
            .field("finished_at", &self.finished_at.load(Ordering::Relaxed))
            .finish()
    }
}

impl ProgressTracker {
    pub fn new(title: Arc<str>, sender: FrontendHandle) -> Self {
        Self {
            inner: Arc::new(ProgressTrackerInner {
                count: AtomicUsize::new(0),
                total: AtomicUsize::new(0),
                finished_at: AtomicOptionInstant::none(),
                finish_type: AtomicProgressTrackerFinishType::new(ProgressTrackerFinishType::Normal),
                title: RwLock::new(title),
            }),
            sender,
            notify_serial: AtomicOptionSerial::default(),
        }
    }

    pub fn id(&self) -> usize {
        Arc::as_ptr(&self.inner).addr()
    }

    pub fn get_title(&self) -> Arc<str> {
        self.inner.title.read().unwrap().clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        *self.inner.title.write().unwrap() = title;
    }

    pub fn get_float(&self) -> Option<f32> {
        let (count, total) = self.get();
        if total == 0 {
            None
        } else {
            Some((count as f32 / total as f32).clamp(0.0, 1.0))
        }
    }

    pub fn get(&self) -> (usize, usize) {
        (
            self.inner.count.load(Ordering::SeqCst),
            self.inner.total.load(Ordering::SeqCst)
        )
    }

    pub fn set_finished(&self, finish_type: ProgressTrackerFinishType) {
        self.inner.finish_type.store(finish_type, Ordering::SeqCst);
        let _ = self.inner.finished_at.compare_exchange(None, Some(Instant::now()), Ordering::SeqCst, Ordering::Relaxed);
    }

    pub fn get_finished_at(&self) -> Option<Instant> {
        self.inner.finished_at.load(Ordering::SeqCst)
    }

    pub fn finish_type(&self) -> ProgressTrackerFinishType {
        self.inner.finish_type.load(Ordering::SeqCst)
    }

    pub fn add_count(&self, count: usize) {
        self.inner.count.fetch_add(count, Ordering::SeqCst);
    }

    pub fn set_count(&self, count: usize) {
        self.inner.count.store(count, Ordering::SeqCst);
    }

    pub fn add_total(&self, total: usize) {
        self.inner.total.fetch_add(total, Ordering::SeqCst);
    }

    pub fn set_total(&self, total: usize) {
        self.inner.total.store(total, Ordering::SeqCst);
    }

    pub fn notify(&self) {
        self.sender.send_with_serial(MessageToFrontend::Refresh, &self.notify_serial);
    }
}
