use std::ffi::{CString, OsStr};
use std::io::Result;
use std::marker::PhantomData;
use std::os::unix::ffi::OsStrExt;

use libc::c_char;

use ffi;
use list::EntryList;
use FromRaw;

/// Rust wrapper for the `udev_hwdb` struct, which provides access to `udev`'s
/// hardware database API.
///
/// Like the `udev` struct, `udev_hwdb` is refcounted and automatically managed
/// by the Rust wrapper.
pub struct Hwdb {
    hwdb: *mut ffi::udev_hwdb,
}

impl Clone for Hwdb {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw(ffi::udev_hwdb_ref(self.hwdb)) }
    }
}

impl Drop for Hwdb {
    fn drop(&mut self) {
        unsafe { ffi::udev_hwdb_unref(self.hwdb) };
    }
}

#[cfg(feature = "send")]
unsafe impl Send for Hwdb {}
#[cfg(feature = "sync")]
unsafe impl Sync for Hwdb {}

as_ffi!(Hwdb, hwdb, ffi::udev_hwdb, ffi::udev_hwdb_ref);

impl Hwdb {
    /// Creates a new Hwdb context.
    pub fn new() -> Result<Self> {
        // NOTE: udev_hwdb_new states that its first parameter is unused.
        // However, older versions of udev check it against NULL, so we can't just pass an
        // empty pointer in. Instead, we pass in a garbage pointer.
        let junk: *mut ffi::udev = 0x41414141_41414141 as *mut ffi::udev;
        let ptr = try_alloc!(unsafe { ffi::udev_hwdb_new(junk) });
        Ok(unsafe { Self::from_raw(ptr) })
    }

    /// Queries the hardware database with the given `modalias` query,
    /// returning an iterator over each matching entry.
    pub fn query<S: AsRef<OsStr>>(&self, modalias: S) -> EntryList<Hwdb> {
        // NOTE: This expect can fail if someone passes a string that contains an internal NUL.
        let modalias = CString::new(modalias.as_ref().as_bytes())
            .expect("query() called with malformed modalias string");
        EntryList {
            entry: unsafe {
                ffi::udev_hwdb_get_properties_list_entry(
                    self.hwdb,
                    modalias.as_ptr() as *const c_char,
                    0,
                )
            },
            phantom: PhantomData,
        }
    }

    /// Returns the first entry value with the given name, or `None` if no result exists.
    pub fn query_one<S: AsRef<OsStr>>(&self, modalias: S, name: S) -> Option<&OsStr> {
        self.query(modalias)
            .find(|e| e.name == name.as_ref())
            .map(|e| e.value.unwrap_or_else(|| OsStr::new("")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query() {
        let hwdb = Hwdb::new().unwrap();
        // Query the hwdb for a device that should always be known:
        // the Linux Foundation's USB 1.1. root hub
        let results: Vec<_> = hwdb.query("usb:v1D6Bp0001").collect();

        assert!(results.len() >= 2);

        // We expect an ID_VENDOR_FROM_DATABASE and an ID_MODEL_FROM_DATABASE with corresponding
        // values; no order is specified by udev.

        assert!(results.iter().any(|e| e.name == "ID_VENDOR_FROM_DATABASE"));
        assert!(results.iter().any(|e| e.name == "ID_MODEL_FROM_DATABASE"));

        assert!(results
            .iter()
            .any(|e| e.value.unwrap_or(OsStr::new("")) == "Linux Foundation"));
        assert!(results
            .iter()
            .any(|e| e.value.unwrap_or(OsStr::new("")) == "1.1 root hub"));
    }

    #[test]
    fn test_query_one() {
        let hwdb = Hwdb::new().unwrap();
        let value = hwdb
            .query_one("usb:v1D6Bp0001", "ID_MODEL_FROM_DATABASE")
            .unwrap();

        assert_eq!(value, "1.1 root hub");
    }
}
