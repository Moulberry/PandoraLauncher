use std::{
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

use atomic_time::AtomicOptionInstant;
use parking_lot::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

#[derive(Default, Clone, Debug)]
pub struct ModalAction(Arc<ModalActionInner>);

impl ModalAction {
    pub fn refcnt(&self) -> usize {
        Arc::strong_count(&self.0)
    }
}

impl Deref for ModalAction {
    type Target = ModalActionInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct ModalActionVisitUrl {
    pub message: Arc<str>,
    pub url: Arc<str>,
    pub prevent_auto_finish: bool,
}

#[derive(Default)]
pub struct ModalActionInner {
    notify: Arc<tokio::sync::Notify>,
    finished_at: AtomicOptionInstant,
    finish_effects: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    error: RwLock<Option<Arc<str>>>,
    visit_url: RwLock<Option<ModalActionVisitUrl>>,
    trackers: Arc<RwLock<Vec<ProgressTracker>>>,
    pub request_cancel: CancellationToken,
}

impl Drop for ModalActionInner {
    fn drop(&mut self) {
        for effect in self.finish_effects.lock().drain(..) {
            (effect)();
        }
    }
}

impl ModalActionInner {
    pub fn add_finish_effect(&self, effect: impl FnOnce() + Send + 'static) {
        self.finish_effects.lock().push(Box::new(effect));
    }

    pub fn get_notify(&self) -> Arc<tokio::sync::Notify> {
        self.notify.clone()
    }

    pub fn set_finished(&self) {
        for effect in self.finish_effects.lock().drain(..) {
            (effect)();
        }
        let _ = self.finished_at.compare_exchange(None, Some(Instant::now()), Ordering::SeqCst, Ordering::Relaxed);
        self.notify.notify_one();
    }

    pub fn get_finished_at(&self) -> Option<Instant> {
        self.finished_at.load(Ordering::SeqCst)
    }

    pub fn set_finished_with_error(&self, error: Arc<str>) {
        *self.error.write() = Some(error);
        self.set_finished();
    }

    pub fn get_error_message(&self) -> Option<Arc<str>> {
        self.error.read().clone()
    }

    pub fn set_visit_url(&self, visit_url: ModalActionVisitUrl) {
        *self.visit_url.write() = Some(visit_url);
        self.notify.notify_one();
    }

    pub fn unset_visit_url(&self) {
        *self.visit_url.write() = None;
        self.notify.notify_one();
    }

    pub fn get_visit_url(&self) -> Option<ModalActionVisitUrl> {
        self.visit_url.read().clone()
    }

    pub fn has_requested_cancel(&self) -> bool {
        self.request_cancel.is_cancelled()
    }

    pub fn push_tracker(&self, title: Arc<str>) -> ProgressTracker {
        let tracker = ProgressTracker(Arc::new(ProgressTrackerInner {
            notify: self.notify.clone(),
            count: AtomicUsize::new(0),
            total: AtomicUsize::new(0),
            finished_at: AtomicOptionInstant::none(),
            finish_type: AtomicProgressTrackerFinishType::new(ProgressTrackerFinishType::Normal),
            title: RwLock::new(title),
        }));

        self.trackers.write().push(tracker.clone());
        self.notify.notify_one();

        tracker
    }

    pub fn clear_trackers(&self) {
        self.trackers.write().clear();
        self.notify.notify_one();
    }

    pub fn write_trackers<R>(&self, f: impl FnOnce(&mut Vec<ProgressTracker>) -> R) -> R {
        let mut guard = self.trackers.write();
        (f)(&mut *guard)
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

#[derive(Clone, Debug)]
pub struct ProgressTracker(Arc<ProgressTrackerInner>);

struct ProgressTrackerInner {
    notify: Arc<tokio::sync::Notify>,
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
    // pub fn id(&self) -> usize {
    //     Arc::as_ptr(&self.0).addr()
    // }

    pub fn get_title(&self) -> Arc<str> {
        self.0.title.read().clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        *self.0.title.write() = title;
        self.0.notify.notify_one();
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
            self.0.count.load(Ordering::SeqCst),
            self.0.total.load(Ordering::SeqCst)
        )
    }

    pub fn set_finished(&self, finish_type: ProgressTrackerFinishType) {
        self.0.finish_type.store(finish_type, Ordering::SeqCst);
        let _ = self.0.finished_at.compare_exchange(None, Some(Instant::now()), Ordering::SeqCst, Ordering::Relaxed);
        self.0.notify.notify_one();
    }

    pub fn get_finished_at(&self) -> Option<Instant> {
        self.0.finished_at.load(Ordering::SeqCst)
    }

    pub fn finish_type(&self) -> ProgressTrackerFinishType {
        self.0.finish_type.load(Ordering::SeqCst)
    }

    pub fn add_count(&self, count: usize) {
        self.0.count.fetch_add(count, Ordering::SeqCst);
        self.0.notify.notify_one();
    }

    pub fn set_count(&self, count: usize) {
        self.0.count.store(count, Ordering::SeqCst);
        self.0.notify.notify_one();
    }

    pub fn add_total(&self, total: usize) {
        self.0.total.fetch_add(total, Ordering::SeqCst);
        self.0.notify.notify_one();
    }

    pub fn set_total(&self, total: usize) {
        self.0.total.store(total, Ordering::SeqCst);
        self.0.notify.notify_one();
    }
}
