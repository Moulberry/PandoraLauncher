use std::{
    collections::HashMap,
    fs::{File, OpenOptions, TryLockError},
    path::Path,
    sync::Arc,
};

use once_cell::sync::Lazy;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Debug)]
pub struct Lockfile {
    _handle: File,
    _permit: OwnedSemaphorePermit,
}

static LOCKED: Lazy<parking_lot::Mutex<HashMap<Arc<Path>, Arc<Semaphore>>>> = Lazy::new(Default::default);

fn get_path_semaphore(path: Arc<Path>) -> Arc<Semaphore> {
    match LOCKED.lock().entry(path) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
        std::collections::hash_map::Entry::Vacant(entry) => entry.insert(Arc::new(Semaphore::new(1))).clone(),
    }
}

fn open_file(path: &Path) -> std::io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.read(true).write(true).create(true);
    open_options.open(&path)
}

impl Lockfile {
    pub async fn create(path: Arc<Path>) -> std::io::Result<Self> {
        let semaphore = get_path_semaphore(path.clone());

        let permit = semaphore.acquire_owned().await.unwrap();

        let mut handle = open_file(&path)?;

        match handle.try_lock() {
            Ok(_) => {},
            Err(TryLockError::Error(err)) => return Err(err),
            Err(TryLockError::WouldBlock) => {
                handle = tokio::task::spawn_blocking(move || {
                    handle.lock()?;
                    std::io::Result::Ok(handle)
                })
                .await??;
            },
        }

        Ok(Self {
            _handle: handle,
            _permit: permit,
        })
    }

    #[expect(unused)]
    pub fn try_create(path: Arc<Path>) -> std::io::Result<Option<Self>> {
        let semaphore = get_path_semaphore(path.clone());

        let Ok(permit) = semaphore.try_acquire_owned() else {
            return Ok(None);
        };

        let handle = open_file(&path)?;

        match handle.try_lock() {
            Ok(_) => {},
            Err(TryLockError::Error(err)) => return Err(err),
            Err(TryLockError::WouldBlock) => {
                return Ok(None);
            },
        }

        Ok(Some(Self {
            _handle: handle,
            _permit: permit,
        }))
    }
}
