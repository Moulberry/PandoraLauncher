#![doc = include_str!("../README.md")]
#![no_std]
#![cfg_attr(target_feature = "atomics", feature(stdarch_wasm_atomic_wait))]
#![allow(clippy::missing_safety_doc)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

extern crate alloc;

mod shim;
#[rustfmt::skip]
#[allow(clippy::type_complexity)]
mod bindings;

/// Low-level utilities, traits, and macros for implementing custom SQLite Virtual File Systems (VFS)
pub mod utils {
    #[doc(inline)]
    pub use rsqlite_vfs::{
        bail, check_db_and_page_size, check_import_db, check_option, check_result, random_name,
        register_vfs, registered_vfs, ImportDbError, MemChunksFile, OsCallback, RegisterVfsError,
        SQLiteIoMethods, SQLiteVfs, SQLiteVfsFile, VfsAppData, VfsError, VfsFile, VfsResult,
        VfsStore, SQLITE3_HEADER,
    };

    pub use rsqlite_vfs::ffi;

    #[doc(hidden)]
    pub use rsqlite_vfs::test_suite;
}

#[doc(inline)]
pub use self::utils::{bail, check_option, check_result};

/// Raw C-style bindings to the underlying `libsqlite3` library.
pub use bindings::*;

/// Wasm platform implementation
pub use self::shim::WasmOsCallback;
/// In-memory VFS implementation.
pub use rsqlite_vfs::memvfs::{MemVfsError, MemVfsUtil};
