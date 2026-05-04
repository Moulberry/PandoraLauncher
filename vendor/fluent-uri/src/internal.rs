#![allow(missing_debug_implementations)]

use alloc::{string::String, vec::Vec};
use core::{
    cell::Cell,
    mem::MaybeUninit,
    num::NonZeroU32,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "std")]
use std::net::{Ipv4Addr, Ipv6Addr};

use super::*;
use bitflags::bitflags;

pub trait Pointer {
    fn get(&self) -> *mut u8;
    fn len(&self) -> u32;
    /// Creates a `Self` from the given (ptr, len, cap) triple.
    ///
    /// # Safety
    ///
    /// - The pointer must not be null.
    /// - The length and capacity must be correct.
    unsafe fn new(ptr: *mut u8, len: u32, cap: u32) -> Self;
    const DANGLING: Self;
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Uncapped {
    ptr: NonNull<u8>,
    len: u32,
    _pad: MaybeUninit<u32>,
}

impl Pointer for Uncapped {
    #[inline]
    fn get(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    unsafe fn new(ptr: *mut u8, len: u32, _cap: u32) -> Self {
        Self {
            // SAFETY: The caller must ensure that the pointer is not null.
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            len,
            _pad: MaybeUninit::uninit(),
        }
    }

    const DANGLING: Self = Self {
        ptr: NonNull::dangling(),
        len: 0,
        _pad: MaybeUninit::uninit(),
    };
}

#[repr(C)]
pub struct Capped {
    ptr: NonNull<u8>,
    len: u32,
    cap: u32,
}

impl Capped {
    #[inline]
    pub fn into_string(self) -> String {
        let me = ManuallyDrop::new(self);
        // SAFETY: `Capped` is created from a `String`.
        unsafe { String::from_raw_parts(me.ptr.as_ptr(), me.len as _, me.cap as _) }
    }
}

impl Pointer for Capped {
    #[inline]
    fn get(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    unsafe fn new(ptr: *mut u8, len: u32, cap: u32) -> Self {
        Self {
            // SAFETY: The caller must ensure that the pointer is not null.
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            len,
            cap,
        }
    }

    const DANGLING: Self = Self {
        ptr: NonNull::dangling(),
        len: 0,
        cap: 0,
    };
}

impl Drop for Capped {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: `Capped` is created from a `String`.
        let _ = unsafe { String::from_raw_parts(self.ptr.as_ptr(), 0, self.cap as _) };
    }
}

pub trait Storage {
    type Ptr: Pointer;
    fn is_mut() -> bool;
}

impl Storage for &str {
    type Ptr = Uncapped;

    #[inline]
    fn is_mut() -> bool {
        false
    }
}

impl Storage for &mut [u8] {
    type Ptr = Uncapped;

    #[inline]
    fn is_mut() -> bool {
        true
    }
}

impl Storage for String {
    type Ptr = Capped;

    #[inline]
    fn is_mut() -> bool {
        false
    }
}

pub trait Io<'i, 'o>: Storage {}

impl<'i, 'a> Io<'i, 'a> for &'a str {}

impl<'a> Io<'a, 'a> for &mut [u8] {}

impl<'a> Io<'a, 'a> for String {}

pub trait IntoOwnedUri {
    fn as_raw_parts(&self) -> (*mut u8, usize, usize);
}

impl IntoOwnedUri for String {
    #[inline]
    fn as_raw_parts(&self) -> (*mut u8, usize, usize) {
        (self.as_ptr() as _, self.len(), self.capacity())
    }
}

impl IntoOwnedUri for Vec<u8> {
    #[inline]
    fn as_raw_parts(&self) -> (*mut u8, usize, usize) {
        (self.as_ptr() as _, self.len(), self.capacity())
    }
}

#[derive(Clone)]
pub struct Data {
    pub tag: Tag,
    // The index of the trailing colon.
    pub scheme_end: Option<NonZeroU32>,
    pub auth: Option<AuthData>,
    pub path_bounds: (u32, u32),
    // One byte past the last byte of query.
    pub query_end: Option<NonZeroU32>,
    // One byte past the preceding '#'.
    pub fragment_start: Option<NonZeroU32>,
}

impl Data {
    pub const INIT: Data = Data {
        tag: Tag::empty(),
        scheme_end: None,
        auth: None,
        path_bounds: (0, 0),
        query_end: None,
        fragment_start: None,
    };
}

#[doc(hidden)]
impl<T: Storage> Deref for Uri<T> {
    type Target = Data;
    #[inline]
    fn deref(&self) -> &Data {
        &self.data
    }
}

#[doc(hidden)]
impl<T: Storage> DerefMut for Uri<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Data {
        &mut self.data
    }
}

bitflags! {
    pub struct Tag: u32 {
        const HOST_REG_NAME = 0b00000001;
        const HOST_IPV4     = 0b00000010;
        const HOST_IPV6     = 0b00000100;
        const AUTH_TAKEN    = 0b00001000;
        const HOST_TAKEN    = 0b00010000;
        const PORT_TAKEN    = 0b00100000;
        const PATH_TAKEN    = 0b01000000;
    }
}

#[derive(Clone)]
pub struct AuthData {
    pub start: Cell<NonZeroU32>,
    pub host_bounds: (u32, u32),
    pub host_data: RawHostData,
}

#[derive(Clone, Copy)]
pub union RawHostData {
    #[cfg(feature = "std")]
    pub ipv4_addr: Ipv4Addr,
    pub ipv6: Ipv6Data,
    #[cfg(feature = "ipv_future")]
    pub ipv_future_dot_i: u32,
    pub none: (),
}

#[derive(Clone, Copy)]
pub struct Ipv6Data {
    #[cfg(feature = "std")]
    pub addr: Ipv6Addr,
    #[cfg(feature = "rfc6874bis")]
    pub zone_id_start: Option<NonZeroU32>,
}
