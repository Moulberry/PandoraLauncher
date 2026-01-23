use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::IoOrSerializationError;

#[derive(Debug)]
pub struct Persistent<T: Serialize + for <'de> Deserialize<'de>> {
    path: Arc<Path>,
    dirty: bool,
    data: T
}

impl<T: Serialize + for <'de> Deserialize<'de> + Default> Persistent<T> {
    pub fn load(path: Arc<Path>) -> Self {
        let data = crate::read_json(&path).unwrap_or_default();
        Self {
            path,
            dirty: false,
            data,
        }
    }
}

impl<T: Serialize + for <'de> Deserialize<'de>> Persistent<T> {
    pub fn try_load(path: Arc<Path>) -> Result<Self, IoOrSerializationError> {
        let data = crate::read_json(&path)?;
        Ok(Self {
            path,
            dirty: false,
            data,
        })
    }

    pub fn load_or(path: Arc<Path>, default_value: T) -> Self {
        let data = crate::read_json(&path).unwrap_or(default_value);
        Self {
            path,
            dirty: false,
            data,
        }
    }

    pub fn modify(&mut self, func: impl FnOnce(&mut T)) {
        if self.dirty {
            self.load_from_disk();
        }

        (func)(&mut self.data);

        if let Ok(bytes) = serde_json::to_vec(&self.data) {
            if crate::write_safe(&self.path, &bytes).is_ok() {
                self.dirty = true;
            }
        }
    }

    pub fn get(&mut self) -> &T {
        if self.dirty {
            self.load_from_disk();
        }

        &self.data
    }

    #[inline(always)]
    pub fn sanity_check_path_eq(&self, path: &Path) {
        debug_assert_eq!(path, &*self.path);
    }

    #[inline(always)]
    pub fn mark_changed(&mut self, path: &Path) {
        self.sanity_check_path_eq(path);
        self.dirty = true;
    }

    fn load_from_disk(&mut self) {
        self.dirty = false;

        let Ok(data) = crate::read_json(&self.path) else {
            return;
        };

        self.data = data;
    }
}
