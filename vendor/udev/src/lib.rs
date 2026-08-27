//!
//! libudev Bindings for Rust
//!

#![warn(missing_docs)]

extern crate io_lifetimes;
extern crate libc;
pub extern crate libudev_sys as ffi;
#[cfg(feature = "mio06")]
pub extern crate mio06 as mio;
#[cfg(feature = "mio07")]
pub extern crate mio07 as mio;
#[cfg(feature = "mio08")]
pub extern crate mio08 as mio;
#[cfg(feature = "mio10")]
pub extern crate mio10 as mio;

pub use device::{Attributes, Device, DeviceType, Properties};
pub use enumerator::{Devices, Enumerator};
#[cfg(feature = "hwdb")]
pub use hwdb::Hwdb;
pub use list::{Entry, List};
pub use monitor::{
    Builder as MonitorBuilder, Event, EventType, Socket as MonitorSocket,
    SocketIter as MonitorSocketIter,
};
pub use udev::Udev;

macro_rules! try_alloc {
    ($exp:expr) => {{
        let ptr = $exp;

        if ptr.is_null() {
            return Err(std::io::Error::last_os_error());
        }

        ptr
    }};
}

/// Receive the underlying raw pointer
pub trait AsRaw<T: 'static> {
    /// Get a reference of the underlying struct.
    ///
    /// The reference count will not be increased.
    fn as_raw(&self) -> *mut T;

    /// Convert the object into the underlying pointer.
    ///
    /// You are responsible for freeing the object.
    fn into_raw(self) -> *mut T;
}

/// Receive the underlying raw pointer for types with an associated `udev` struct which must
/// outlive them.
pub trait AsRawWithContext<T: 'static> {
    /// Get a reference of the underlying struct.
    ///
    /// The reference count will not be increased.
    fn as_raw(&self) -> *mut T;

    /// The `udev` context with which this struct was created.  This must live at least as long as
    /// the struct itself or undefined behavior will result.
    fn udev(&self) -> &Udev;

    /// Convert the object into the raw `udev` pointer and the underlying pointer for this object.
    ///
    /// You are responsible for freeing both.  You're also responsible for ensuring that the `udev`
    /// pointer is not freed until after this object's pointer is freed.
    fn into_raw_with_context(self) -> (*mut ffi::udev, *mut T);
}

/// Convert from a raw pointer
pub trait FromRaw<T: 'static> {
    /// Create an object from a given raw pointer.
    ///
    /// The reference count will not be increased, be sure not to free this pointer.
    ///
    /// ## Safety
    ///
    /// The pointer has to be a valid reference to the expected underlying udev-struct or undefined
    /// behaviour might occur.
    unsafe fn from_raw(ptr: *mut T) -> Self;
}

/// Convert from a raw pointer for types which must be associated with a `Udev` context object.
pub trait FromRawWithContext<T: 'static> {
    /// Create an object from a given raw pointer and `udev` context pointer.
    ///
    /// The reference count will not be increased, be sure not to free this pointer.
    ///
    /// ## Safety
    ///
    /// The `udev` pointer must correspond to the `udev` pointer used when `ptr` was created.  If
    /// not memory corruption and undefined behavior will result.
    ///
    /// Both the `udev` and `ptr` pointers must be a valid reference to the expected underlying udev-struct or undefined
    /// behaviour might occur.  Do NOT attempt to free either pointer; `udev_unref` and the
    /// corresponding `*_unref` function for `ptr` will be called automatically when this type is
    /// dropped.
    unsafe fn from_raw_with_context(udev: *mut ffi::udev, ptr: *mut T) -> Self;
}

/// Convert from a raw pointer and the matching context
macro_rules! as_ffi {
    ($struct_:ident, $field:ident, $type_:ty, $ref:path) => {
        as_raw!($struct_, $field, $type_, $ref);
        from_raw!($struct_, $field, $type_);
    };
}

macro_rules! as_ffi_with_context {
    ($struct_:ident, $field:ident, $type_:ty, $ref:path) => {
        as_raw_with_context!($struct_, $field, $type_, $ref);
        from_raw_with_context!($struct_, $field, $type_);
    };
}

macro_rules! as_raw {
    ($struct_:ident, $field:ident, $type_:ty, $ref:path) => {
        impl $crate::AsRaw<$type_> for $struct_ {
            fn as_raw(&self) -> *mut $type_ {
                self.$field
            }

            fn into_raw(self) -> *mut $type_ {
                // Note that all `AsRaw` implementations also implement `Drop` which calls the
                // `_unref` function that correponds to $type_.  We can't prevent this from
                // happening, so we have to add a reference here to ensure the returned pointer
                // remains allocated for the caller.
                unsafe { $ref(self.$field) };

                self.$field
            }
        }
    };
}

macro_rules! from_raw {
    ($struct_:ident, $field:ident, $type_:ty) => {
        impl $crate::FromRaw<$type_> for $struct_ {
            unsafe fn from_raw(t: *mut $type_) -> Self {
                Self { $field: t }
            }
        }
    };
}

macro_rules! as_raw_with_context {
    ($struct_:ident, $field:ident, $type_:ty, $ref:path) => {
        impl $crate::AsRawWithContext<$type_> for $struct_ {
            fn as_raw(&self) -> *mut $type_ {
                self.$field
            }

            fn udev(&self) -> &Udev {
                &self.udev
            }

            fn into_raw_with_context(self) -> (*mut ffi::udev, *mut $type_) {
                // We can't call `self.udev.into_raw()` here, because that will consume
                // `self.udev`, which is not possible because every type that implements
                // `AsRawWithContext` also implements `Drop`.  Of course we know that it would be
                // safe here to just skip the `drop()` on `Udev` and "leak" the `udev` pointer back
                // to the caller, but the Rust compiler doesn't know that.
                //
                // So instead we have to add a new reference to the `udev` pointer before we return
                // it, because as soon as we leave the scope of this function the `Udev` struct
                // will be dropped which will call `udev_unref` on it.  If there's only once
                // reference left that will free the pointer and we'll end up returning a dangling
                // pointer to the caller.
                //
                // For much the same reason, we do the same with the pointer of type $type
                let udev = self.udev.as_raw();
                unsafe { ffi::udev_ref(udev) };

                unsafe { $ref(self.$field) };

                (udev, self.$field)
            }
        }
    };
}

macro_rules! from_raw_with_context {
    ($struct_:ident, $field:ident, $type_:ty) => {
        impl $crate::FromRawWithContext<$type_> for $struct_ {
            unsafe fn from_raw_with_context(udev: *mut ffi::udev, t: *mut $type_) -> Self {
                Self {
                    udev: Udev::from_raw(udev),
                    $field: t,
                }
            }
        }
    };
}

mod device;
mod enumerator;
#[cfg(feature = "hwdb")]
mod hwdb;
mod list;
mod monitor;
mod udev;
mod util;
