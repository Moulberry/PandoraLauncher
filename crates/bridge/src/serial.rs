use std::{
    cmp::Ordering,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize},
    },
};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Serial(usize);

impl Serial {
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

impl PartialOrd for Serial {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let distance = self.0.abs_diff(other.0);
        if distance < usize::MAX / 2 {
            self.0.partial_cmp(&other.0)
        } else {
            other.0.partial_cmp(&self.0)
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct AtomicSetSerial(pub(crate) Arc<AtomicUsize>);

impl AtomicSetSerial {
    pub fn set(&self, serial: Serial) {
        self.0.store(serial.0, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn get(&self) -> Serial {
        Serial(self.0.load(std::sync::atomic::Ordering::Relaxed))
    }
}

#[derive(Default, Debug, Clone)]
pub struct AtomicSerialProvider(Arc<AtomicUsize>);

impl AtomicSerialProvider {
    pub fn next(&self) -> Serial {
        Serial(self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed).wrapping_add(1))
    }
}

#[derive(Default, Debug, Clone)]
pub struct AtomicOptionSerial(Arc<(AtomicUsize, AtomicBool)>);

impl AtomicOptionSerial {
    pub(crate) fn set(&self, serial: Serial) {
        self.0.0.store(serial.0, std::sync::atomic::Ordering::SeqCst);
        self.0.1.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn get(&self) -> Option<Serial> {
        if self.0.1.load(std::sync::atomic::Ordering::SeqCst) {
            return Some(Serial(self.0.0.load(std::sync::atomic::Ordering::SeqCst)));
        } else {
            return None;
        }
    }
}
