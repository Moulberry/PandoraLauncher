use std::{ffi::{CString, OsString}, io::ErrorKind, os::unix::ffi::OsStringExt};

use libc::c_char;

#[doc(hidden)]
pub trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
    ($($t:ident)*) => ($(impl IsMinusOne for $t {
        fn is_minus_one(&self) -> bool {
            *self == -1
        }
    })*)
}

impl_is_minus_one! { i8 i16 i32 i64 isize }

/// Converts native return values to Result using the *-1 means error is in `errno`*  convention.
/// Non-error values are `Ok`-wrapped.
pub fn cvt<T: IsMinusOne>(t: T) -> std::io::Result<T> {
    if t.is_minus_one() { Err(std::io::Error::last_os_error()) } else { Ok(t) }
}

/// `-1` → look at `errno` → retry on `EINTR`. Otherwise `Ok()`-wrap the closure return value.
pub fn cvt_r<T, F>(mut f: F) -> std::io::Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    loop {
        match cvt(f()) {
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

#[cfg(target_vendor = "apple")]
pub unsafe fn environ() -> *mut *const *const c_char {
    unsafe { libc::_NSGetEnviron() as *mut *const *const c_char }
}

// Use the `environ` static which is part of POSIX.
#[cfg(not(target_vendor = "apple"))]
pub unsafe fn environ() -> *mut *const *const c_char {
    unsafe extern "C" {
        static mut environ: *const *const c_char;
    }
    &raw mut environ
}

#[derive(Debug)]
pub struct RawStringVec(Vec<*mut c_char>);

impl RawStringVec {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity + 1))
    }

    pub fn push_c(&mut self, string: CString) {
        self.0.push(string.into_raw());
    }

    pub fn push_os(&mut self, string: OsString) -> std::io::Result<()> {
        self.push_c(CString::new(string.into_vec())?);
        Ok(())
    }

    pub fn into_null_terminated_ptr(mut self) -> *const *mut c_char {
        assert!(self.0.last().unwrap().is_null());
        std::mem::take(&mut self.0).into_raw_parts().0
    }

    #[cfg(target_os = "macos")]
    pub fn as_null_terminated_ptr(&self) -> *const *mut c_char {
        assert!(self.0.last().unwrap().is_null());
        self.0.as_ptr()
    }

    pub fn ensure_null_terminated(&mut self) {
        if let Some(last) = self.0.last() && last.is_null() {
            return;
        }
        self.0.push(std::ptr::null_mut());
    }
}

unsafe impl Send for RawStringVec {}
unsafe impl Sync for RawStringVec {}

impl Drop for RawStringVec {
    fn drop(&mut self) {
        for ptr in self.0.drain(..) {
            if !ptr.is_null() {
                drop(unsafe { CString::from_raw(ptr) });
            }
        }
    }
}
