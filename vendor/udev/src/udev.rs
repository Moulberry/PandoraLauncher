use std::io::Result;

use ffi;

use FromRaw;

/// Rust wrapper for the `udev` struct which represents an opaque libudev context
///
/// Most other `libudev` calls take a `struct udev*` argument, although whether or not this
/// argument is actually used depends on the version of libudev.  In more recent versions the
/// context is ignored, therefore it sometimes works to pass a NULL or a invalid pointer for
/// `udev`.  However older versions, specifically 215 which shipped with Debian 8, expect this to
/// be a valid `udev` struct.  Thus it is not optional.
///
/// `udev` is a ref-counted struct, with references added and removed with `udev_ref` and
/// `udef_unref` respectively.  This Rust wrapper takes advantage of that ref counting to implement
/// `Clone` and `Drop`, so callers need not worry about any C-specific resource management.
pub struct Udev {
    udev: *mut ffi::udev,
}

impl Clone for Udev {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw(ffi::udev_ref(self.udev)) }
    }
}

impl Drop for Udev {
    fn drop(&mut self) {
        unsafe { ffi::udev_unref(self.udev) };
    }
}

#[cfg(feature = "send")]
unsafe impl Send for Udev {}
#[cfg(feature = "sync")]
unsafe impl Sync for Udev {}

as_ffi!(Udev, udev, ffi::udev, ffi::udev_ref);

impl Udev {
    /// Creates a new Udev context.
    pub fn new() -> Result<Self> {
        let ptr = try_alloc!(unsafe { ffi::udev_new() });
        Ok(unsafe { Self::from_raw(ptr) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AsRaw;

    #[test]
    fn clone_drop() {
        // Exercise clone/drop.  We won't be able to catch a bug here that leaks memory, but a
        // crash due to the ref count getting out of whack would show up here.
        let mut udev = Udev::new().unwrap();

        for _ in 0..1000 {
            let clone = udev.clone();

            assert_eq!(udev.as_raw(), clone.as_raw());

            // This will `drop()` what's in `udev`, and transfer ownership from `clone` to `udev`
            udev = clone;
        }
    }

    #[test]
    fn round_trip_to_raw_pointers() {
        // Make sure this can be made into a raw pointer, then back to a Rust type, and still works
        let udev = Udev::new().unwrap();

        let ptr = udev.into_raw();

        let udev = unsafe { Udev::from_raw(ptr) };

        assert_eq!(ptr, udev.as_raw());
    }
}
