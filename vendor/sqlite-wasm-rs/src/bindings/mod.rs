//! This module is codegen from build.rs. Avoid manual edits.

#[cfg(all(not(feature = "bindgen"), feature = "sqlite3mc"))]
mod sqlite3mc_bindgen;

#[cfg(all(not(feature = "bindgen"), not(feature = "sqlite3mc")))]
mod sqlite3_bindgen;

mod bindgen {
    #[cfg(feature = "bindgen")]
    include!(concat!(env!("OUT_DIR"), "/bindgen.rs"));

    #[cfg(all(not(feature = "bindgen"), feature = "sqlite3mc"))]
    pub use super::sqlite3mc_bindgen::*;

    #[cfg(all(not(feature = "bindgen"), not(feature = "sqlite3mc")))]
    pub use super::sqlite3_bindgen::*;
}

mod error;

pub use bindgen::*;
pub use error::*;

use core::mem;

#[must_use]
pub fn SQLITE_STATIC() -> sqlite3_destructor_type {
    None
}

#[must_use]
pub fn SQLITE_TRANSIENT() -> sqlite3_destructor_type {
    // SQLite uses -1 as a sentinel for "make your own copy".
    Some(unsafe {
        mem::transmute::<isize, unsafe extern "C" fn(*mut core::ffi::c_void)>(-1_isize)
    })
}

impl Default for sqlite3_vtab {
    fn default() -> Self {
        // C expects zero-initialized vtab structs.
        unsafe { mem::zeroed() }
    }
}

impl Default for sqlite3_vtab_cursor {
    fn default() -> Self {
        // C expects zero-initialized vtab cursor structs.
        unsafe { mem::zeroed() }
    }
}
