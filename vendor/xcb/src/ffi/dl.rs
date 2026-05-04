/*
 * Copyright (C) 2013 James Miller <james@aatch.net>
 * Copyright (c) 2016
 *         Remi Thebault <remi.thebault@gmail.com>
 *         Thomas Bracht Laumann Jespersen <laumann.thomas@gmail.com>
 * Copyright (c) 2025 Tolga Mizrak <tolga.mizrak@gmail.com>
 *
 * Permission is hereby granted, free of charge, to any
 * person obtaining a copy of this software and associated
 * documentation files (the "Software"), to deal in the
 * Software without restriction, including without
 * limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software
 * is furnished to do so, subject to the following
 * conditions:
 *
 * The above copyright notice and this permission notice
 * shall be included in all copies or substantial portions
 * of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 * ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
 * TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
 * PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
 * SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
 * OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

#[cfg(feature = "dl")]
mod dl_impl {
    use std::error::Error;
    use std::fmt::{Display, Formatter};
    use std::sync::PoisonError;

    #[derive(Debug)]
    pub enum OpenError {
        Library(libloading::Error),
        InternalError,
    }

    impl Display for OpenError {
        fn fmt(&self, f: &mut Formatter) -> Result<(), ::std::fmt::Error> {
            let detail = match self {
                OpenError::Library(detail) => {
                    f.write_str("Library")?;
                    detail.to_string()
                }
                OpenError::InternalError => {
                    f.write_str("InternalError")?;
                    String::new()
                }
            };
            if !detail.is_empty() {
                f.write_fmt(format_args!(" ({})", detail))?;
            }
            Ok(())
        }
    }

    impl Error for OpenError {}

    impl From<libloading::Error> for OpenError {
        fn from(value: libloading::Error) -> Self {
            OpenError::Library(value)
        }
    }

    impl<T> From<PoisonError<T>> for OpenError {
        fn from(_: PoisonError<T>) -> Self {
            OpenError::InternalError
        }
    }

    pub unsafe fn load_library(
        prefix: Option<&str>,
        names: &[&str],
    ) -> Result<libloading::Library, libloading::Error> {
        use std::path::{Path, PathBuf};

        debug_assert!(!names.is_empty());
        let mut last_error = None;

        if prefix.is_some() {
            match load_library(None, names) {
                Ok(lib) => return Ok(lib),
                Err(err) => last_error = Some(err),
            }
        }

        for name in names {
            let realpath = match prefix {
                Some(prefix) => Path::new(prefix).join(name),
                None => PathBuf::from(name),
            };
            match libloading::Library::new(realpath) {
                Ok(lib) => return Ok(lib),
                Err(err) => last_error = Some(err),
            }
        }

        Err(last_error.unwrap())
    }

    macro_rules! xcb_get_funcs {
        ($($name:ident),*) => {
            let lib = crate::ffi::XcbLib::open()?;
            $(
                let $name = lib.$name;
            )*
        };
    }
    pub(crate) use xcb_get_funcs;
}

#[cfg(feature = "dl")]
pub(crate) use dl_impl::*;

/// This macro either defines extern functions to link to, or a wrapper around a dynamically loaded library,
/// depending on whether the crate is build with the "dl" flag or not.
/// The wrapper struct is shareable (through clone()) and will keep the function pointers alive,
/// as long as the struct lives.
#[cfg(feature = "dl")]
macro_rules! define_api_dynamic {
    (
        $(#[$smeta:meta])*
        $svis:vis $sname:ident $cache:ident
        libs: [$($lname:literal),+]
        link: $link_name:literal
        functions: $($(#[$meta:meta])* $fvis:vis fn $fname:ident($($farg:ident : $ftype:ty),*$(,)?) $(-> $fret:ty)?);+;
    ) => {
        $(#[$smeta])*
        #[derive(Clone)]
        $svis struct $sname {
            // By putting the library handle into an Arc we allow this struct to be shareable,
            // but don't pay for the atomic load when just using the function pointers.
            _lib: std::sync::Arc<libloading::Library>,
            $(
                $(#[$meta])*
                $fvis $fname: unsafe extern "C" fn($($farg : $ftype),*) $(-> $fret)?
            ),+,
        }

        // Global cache so that if the library is already loaded, we just return a reference
        // counted clone.
        static $cache: std::sync::RwLock<Option<$sname>> = std::sync::RwLock::new(None);

        impl $sname {
            $svis fn open() -> Result<Self, crate::ffi::dl::OpenError> {
                // Try to read at first, whether the library was already loaded.
                {
                    let lock = $cache.read()?;
                    if let Some(lib) = lock.as_ref() {
                        return Ok(lib.clone());
                    }
                }

                let mut lock = $cache.write()?;
                // Between the read and write lock, someone might have already loaded the library.
                if let Some(lib) = lock.as_ref() {
                    return Ok(lib.clone());
                }

                let lib = unsafe {
                    let lib = std::sync::Arc::new(crate::ffi::dl::load_library(None, &[$($lname),+])?);
                    Self {
                        $(
                            $fname: *lib.get(concat!(stringify!($fname), "\0").as_bytes())?
                        ),+,
                        _lib: lib,
                    }
                };
                *lock = Some(lib.clone());
                Ok(lib)
            }

            /// Unloads the current cached instance of the library.
            /// Doesn't prevent another open of the library and recreation of the cache,
            /// if they are used again after unloading.
            pub fn unload() -> Result<(), crate::ffi::dl::OpenError> {
                let mut lock = $cache.write()?;
                *lock = None;
                Ok(())
            }
        }
    };
}

#[cfg(feature = "dl")]
pub(crate) use define_api_dynamic;

#[cfg(feature = "dl")]
macro_rules! xcb_get_funcs_expect {
    ($($name:ident),*) => {
        let lib = crate::ffi::XcbLib::open().expect("xcb library not loaded");
        $(
            let $name = lib.$name;
        )*
    };
}
#[cfg(feature = "dl")]
pub(crate) use xcb_get_funcs_expect;

#[cfg(not(feature = "dl"))]
macro_rules! define_api_link {
    (
        $(#[$_:meta])*
        $svis:vis $sname:ident $cache:ident
        libs: [$($lname:literal),+]
        link: $link_name:literal
        functions: $($(#[$meta:meta])* $fvis:vis fn $fname:ident($($farg:ident : $ftype:ty),*$(,)?) $(-> $fret:ty)?);+;
    ) => {
        #[link(name = $link_name)]
        extern "C" {
            $(
                $(#[$meta])*
                $fvis fn $fname($($farg : $ftype),*) $(-> $fret)?;
            )+
        }
    };
}

#[cfg(not(feature = "dl"))]
pub(crate) use define_api_link;
