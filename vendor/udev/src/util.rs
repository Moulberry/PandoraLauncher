use std::ffi::{CStr, CString, OsStr};
use std::io::Result;

use libc::{c_char, c_int};

use std::os::unix::prelude::*;

pub unsafe fn ptr_to_os_str<'a>(ptr: *const c_char) -> Option<&'a OsStr> {
    if ptr.is_null() {
        return None;
    }

    Some(ptr_to_os_str_unchecked(ptr))
}

pub unsafe fn ptr_to_os_str_unchecked<'a>(ptr: *const c_char) -> &'a OsStr {
    OsStr::from_bytes(CStr::from_ptr(ptr).to_bytes())
}

pub fn os_str_to_cstring<T: AsRef<OsStr>>(s: T) -> Result<CString> {
    match CString::new(s.as_ref().as_bytes()) {
        Ok(s) => Ok(s),
        Err(_) => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
    }
}

pub fn errno_to_result(errno: c_int) -> Result<()> {
    match errno {
        x if x >= 0 => Ok(()),
        e => Err(std::io::Error::from_raw_os_error(-e)),
    }
}
