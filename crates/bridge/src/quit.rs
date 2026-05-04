use std::sync::Arc;

use parking_lot::Mutex;

struct QuitCoordinatorInner {
    mask: u32,
    can_quit_state: u32,
    on_quit: Option<Box<dyn FnOnce() + Send + Sync>>,
}

#[derive(Clone)]
pub struct QuitCoordinator {
    index: u32,
    shared: Arc<Mutex<QuitCoordinatorInner>>,
}

impl Drop for QuitCoordinator {
    fn drop(&mut self) {
        self.shared.lock().mask &= !(1 << self.index);
    }
}

impl QuitCoordinator {
    pub fn new(on_quit: Box<dyn FnOnce() + Send + Sync>) -> Self {
        let inner = QuitCoordinatorInner {
            mask: 1,
            can_quit_state: 0,
            on_quit: Some(on_quit),
        };
        Self {
            index: 0,
            shared: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn fork(&self) -> Self {
        let mut guard = self.shared.lock();
        let index = guard.mask.trailing_ones();
        if index >= u32::BITS {
            panic!("QuitCoordinator only supports up to 32 forks");
        }
        guard.mask |= 1 << index;
        guard.can_quit_state &= !(1 << index);
        drop(guard);
        Self {
            index,
            shared: self.shared.clone()
        }
    }

    pub fn set_can_quit(&self, can_quit: bool) {
        let mut guard = self.shared.lock();
        if can_quit {
            guard.can_quit_state |= 1 << self.index;
            if (guard.can_quit_state & guard.mask) == guard.mask {
                if let Some(on_quit) = guard.on_quit.take() {
                    (on_quit)();
                }
            }
        } else {
            guard.can_quit_state &= !(1 << self.index);
        }
    }
}
