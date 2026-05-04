use crate::legacy::internal::pixel::{ComponentMap, ColorComponentMap};
#[allow(deprecated)]
use crate::legacy::internal::pixel::ComponentSlice;
use core::slice;

pub use crate::formats::gray::Gray_v08 as Gray;
pub use crate::formats::gray_alpha::GrayAlpha_v08 as GrayAlpha;

/// Renamed to `Bgra`
#[doc(hidden)]
pub use crate::formats::bgra::Bgra as BGRA;

/// Renamed to `Bgr`
#[doc(hidden)]
pub use crate::formats::bgr::Bgr as BGR;

/// Renamed to `Abgr`
#[doc(hidden)]
pub use crate::formats::abgr::Abgr as ABGR;

/// Renamed to `Argb`
#[doc(hidden)]
pub use crate::formats::argb::Argb as ARGB;

/// Renamed to `Grb`
#[doc(hidden)]
pub use crate::formats::grb::Grb as GRB;

/// 8-bit BGR
pub type BGR8 = crate::formats::bgr::Bgr<u8>;

/// 16-bit BGR in machine's native endian
pub type BGR16 = crate::formats::bgr::Bgr<u16>;

/// 8-bit BGRA
pub type BGRA8 = crate::formats::bgra::Bgra<u8>;

/// 8-bit ABGR, alpha is first. 0 = transparent, 255 = opaque.
pub type ABGR8 = crate::formats::abgr::Abgr<u8>;

/// 8-bit ARGB, alpha is first. 0 = transparent, 255 = opaque.
pub type ARGB8 = crate::Argb<u8>;

/// 16-bit BGR in machine's native endian
pub type BGRA16 = crate::formats::bgra::Bgra<u16>;

/// 16-bit ABGR in machine's native endian. 0 = transparent, 65535 = opaque.
pub type ABGR16 = crate::formats::abgr::Abgr<u16>;

/// 16-bit ARGB in machine's native endian. 0 = transparent, 65535 = opaque.
pub type ARGB16 = crate::Argb<u16>;

/// 8-bit GRB
pub type GRB8 = crate::formats::grb::Grb<u8>;

/// 8-bit gray
#[deprecated(note = "Refer to ::rgb::alt::Gray<u8> directly (this type alias will change in the next major version)")]
pub type GRAY8 = Gray<u8>;

/// 16-bit gray in machine's native endian
#[deprecated(note = "Refer to ::rgb::alt::Gray<u16> directly (this type alias will change in the next major version)")]
pub type GRAY16 = Gray<u16>;

/// 8-bit gray with alpha in machine's native endian
#[deprecated(note = "Refer to ::rgb::alt::GrayAlpha<u8> directly (this type alias will change in the next major version)")]
pub type GRAYA8 = GrayAlpha<u8>;

/// 16-bit gray with alpha in machine's native endian
#[deprecated(note = "Refer to ::rgb::alt::GrayAlpha<u16> directly (this type alias will change in the next major version)")]
pub type GRAYA16 = GrayAlpha<u16>;


#[cfg(not(feature = "unstable-experimental"))]
impl<T> core::ops::Deref for Gray<T> {
    type Target = T;

    #[inline(always)]
    #[allow(deprecated)]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Copy> From<T> for Gray<T> {
    #[inline(always)]
    fn from(component: T) -> Self {
        Self(component)
    }
}

impl<T: Clone, A> GrayAlpha<T, A> {
    /// Copy `Gray` component out of the `GrayAlpha` struct
    #[inline(always)]
    #[allow(deprecated)]
    pub fn gray(&self) -> Gray<T> {
        Gray(self.0.clone())
    }
}

impl<T, A> GrayAlpha<T, A> {
    /// Provide a mutable view of only `Gray` component (leaving out alpha).
    #[inline(always)]
    pub fn gray_mut(&mut self) -> &mut Gray<T> {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}

impl<T: Copy, A: Clone> GrayAlpha<T, A> {
    /// Create a new `GrayAlpha` with the new alpha value, but same gray value
    #[doc(hidden)]
    #[deprecated(note = "use .with_alpha(a) instead; this will become a getter in the future")]
    pub fn alpha(&self, a: A) -> Self {
        self.with_alpha(a)
    }

    /// Create a new `GrayAlpha` with the new alpha value, but same gray value
    #[inline(always)]
    #[allow(deprecated)]
    pub fn with_alpha(&self, a: A) -> Self {
        Self(self.0, a)
    }

    /// Create a new `GrayAlpha` with a new alpha value created by the callback.
    #[inline(always)]
    #[allow(deprecated)]
    pub fn map_alpha<F, B>(&self, f: F) -> GrayAlpha<T, B>
        where F: FnOnce(A) -> B
    {
        GrayAlpha(self.0, f(self.1.clone()))
    }

    /// Create new `GrayAlpha` with the same alpha value, but different `Gray` value
    #[inline(always)]
    #[allow(deprecated)]
    pub fn map_gray<F, U, B>(&self, f: F) -> GrayAlpha<U, B>
        where F: FnOnce(T) -> U, U: Clone, B: From<A> + Clone {
        GrayAlpha(f(self.0), self.1.clone().into())
    }
}

impl<T: Copy, B> ComponentMap<Gray<B>, T, B> for Gray<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn map<F>(&self, mut f: F) -> Gray<B> where F: FnMut(T) -> B {
        Gray(f(self.0))
    }
}

impl<T: Copy, B> ColorComponentMap<Gray<B>, T, B> for Gray<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn map_colors<F>(&self, mut f: F) -> Gray<B> where F: FnMut(T) -> B {
        Gray(f(self.0))
    }
}

impl<T: Copy, B> ComponentMap<GrayAlpha<B>, T, B> for GrayAlpha<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn map<F>(&self, mut f: F) -> GrayAlpha<B>
    where F: FnMut(T) -> B {
        GrayAlpha(f(self.0), f(self.1))
    }
}

impl<T: Copy, A: Copy, B> ColorComponentMap<GrayAlpha<B, A>, T, B> for GrayAlpha<T, A> {
    #[inline(always)]
    #[allow(deprecated)]
    fn map_colors<F>(&self, mut f: F) -> GrayAlpha<B, A>
    where F: FnMut(T) -> B {
        GrayAlpha(f(self.0), self.1)
    }
}

#[allow(deprecated)]
impl<T> ComponentSlice<T> for GrayAlpha<T> {
    #[inline(always)]
    fn as_slice(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts((self as *const Self).cast::<T>(), 2)
        }
    }

    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut((self as *mut Self).cast::<T>(), 2)
        }
    }
}

#[allow(deprecated)]
impl<T> ComponentSlice<T> for [GrayAlpha<T>] {
    #[inline]
    fn as_slice(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.as_ptr().cast(), self.len() * 2)
        }
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.as_mut_ptr().cast::<T>(), self.len() * 2)
        }
    }
}

#[allow(deprecated)]
impl<T> ComponentSlice<T> for Gray<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn as_slice(&self) -> &[T] {
        slice::from_ref(&self.0)
    }

    #[inline(always)]
    #[allow(deprecated)]
    fn as_mut_slice(&mut self) -> &mut [T] {
        slice::from_mut(&mut self.0)
    }
}

#[allow(deprecated)]
impl<T> ComponentSlice<T> for [Gray<T>] {
    #[inline]
    fn as_slice(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(self.as_ptr().cast(), self.len())
        }
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(self.as_mut_ptr().cast::<T>(), self.len())
        }
    }
}

/// Assumes 255 is opaque
impl<T: Copy> From<Gray<T>> for GrayAlpha<T, u8> {
    #[inline(always)]
    #[allow(deprecated)]
    fn from(other: Gray<T>) -> Self {
        Self(other.0, 0xFF)
    }
}

/// Assumes 65535 is opaque
impl<T: Copy> From<Gray<T>> for GrayAlpha<T, u16> {
    #[inline(always)]
    #[allow(deprecated)]
    fn from(other: Gray<T>) -> Self {
        Self(other.0, 0xFFFF)
    }
}
