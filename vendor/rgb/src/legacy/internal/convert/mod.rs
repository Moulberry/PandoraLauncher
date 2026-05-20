#[allow(deprecated)]
use super::pixel::ComponentSlice;
use super::pixel::ComponentMap;
use crate::alt::{BGR, BGRA, GRB, ARGB, ABGR};
use crate::{RGB, RGBA, Gray, GrayAlpha};
use core::{mem, slice};

mod array;
mod tuple;

/// Use [`::bytemuck::cast_slice()`] instead.
///
/// Casts a slice of bytes into a slice of pixels, e.g. `[u8]` to `[RGB8]`.
///
/// See also `FromSlice`
pub trait AsPixels<PixelType> {
    /// Use [`::bytemuck::cast_slice()`] instead.
    ///
    /// Reinterpret the slice as a read-only/shared slice of pixels.
    /// Multiple consecutive elements in the slice are intepreted as a single pixel
    /// (depending on format, e.g. 3 for RGB, 4 for RGBA).
    ///
    /// Leftover elements are ignored if the slice isn't evenly divisible into pixels.
    ///
    /// Use this method only when the type is known from context.
    /// See also `FromSlice`.
    fn as_pixels(&self) -> &[PixelType];
    /// Use [`::bytemuck::cast_slice_mut()`] instead.
    ///
    /// Reinterpret the slice as a mutable/exclusive slice of pixels.
    /// Multiple consecutive elements in the slice are intepreted as a single pixel
    /// (depending on format, e.g. 3 for RGB, 4 for RGBA).
    ///
    /// Leftover elements are ignored if the slice isn't evenly divisible into pixels.
    ///
    /// Use this method only when the type is known from context.
    /// See also `FromSlice`.
    fn as_pixels_mut(&mut self) -> &mut [PixelType];
}

macro_rules! as_pixels_impl {
    ($typ:ident, $elems:expr) => {
        impl<T> AsPixels<$typ<T>> for [T] {
            fn as_pixels(&self) -> &[$typ<T>] {
                unsafe {
                    slice::from_raw_parts(self.as_ptr() as *const _, self.len() / $elems)
                }
            }

            fn as_pixels_mut(&mut self) -> &mut [$typ<T>] {
                unsafe {
                    slice::from_raw_parts_mut(self.as_mut_ptr() as *mut _, self.len() / $elems)
                }
            }
        }
    };
}

as_pixels_impl! {RGB, 3}
as_pixels_impl! {RGBA, 4}
as_pixels_impl! {BGR, 3}
as_pixels_impl! {BGRA, 4}
as_pixels_impl! {GRB, 3}
as_pixels_impl! {Gray, 1}
as_pixels_impl! {GrayAlpha, 2}
as_pixels_impl! {ARGB, 4}
as_pixels_impl! {ABGR, 4}

/// Use [`::bytemuck::cast_slice()`] or [`::bytemuck::from_bytes()`] to convert
///
/// Cast a slice of component values (bytes) as a slice of RGB/RGBA pixels
///
/// If there's any incomplete pixel at the end of the slice it is ignored.
pub trait FromSlice<T: Copy> {
    /// Reinterpert slice as RGB pixels
    fn as_rgb(&self) -> &[RGB<T>];
    /// Reinterpert slice as RGBA pixels
    fn as_rgba(&self) -> &[RGBA<T>];
    /// Reinterpert slice as alpha-first ARGB pixels
    fn as_argb(&self) -> &[ARGB<T>];
    /// Reinterpert mutable slice as RGB pixels
    fn as_rgb_mut(&mut self) -> &mut [RGB<T>];
    /// Reinterpert mutable slice as RGBA pixels
    fn as_rgba_mut(&mut self) -> &mut [RGBA<T>];
    /// Reinterpert mutable slice as alpha-first ARGB pixels
    fn as_argb_mut(&mut self) -> &mut [ARGB<T>];

    /// Reinterpert mutable slice as grayscale pixels
    fn as_gray(&self) -> &[Gray<T>];
    /// Reinterpert mutable slice as grayscale pixels with alpha
    fn as_gray_alpha(&self) -> &[GrayAlpha<T>];
    /// Reinterpert mutable slice as grayscale pixels
    fn as_gray_mut(&mut self) -> &mut [Gray<T>];
    /// Reinterpert mutable slice as grayscale pixels with alpha
    fn as_gray_alpha_mut(&mut self) -> &mut [GrayAlpha<T>];

    /// Reinterpert slice as reverse-order BGR pixels
    fn as_bgr(&self) -> &[BGR<T>];
    /// Reinterpert slice as reverse-order BGRA pixels
    fn as_bgra(&self) -> &[BGRA<T>];
    /// Reinterpert slice as reverse-order ABGR pixels
    fn as_abgr(&self) -> &[ABGR<T>];
    /// Reinterpert ntable slice as reverse-order BGR pixels
    fn as_bgr_mut(&mut self) -> &mut [BGR<T>];
    /// Reinterpert mutable slice as reverse-order alpha-last BGRA pixels
    fn as_bgra_mut(&mut self) -> &mut [BGRA<T>];
    /// Reinterpert mutable slice as reverse-order alpha-first ABGR pixels
    fn as_abgr_mut(&mut self) -> &mut [ABGR<T>];
}

impl<T: Copy> FromSlice<T> for [T] {
    #[inline]
    fn as_rgb(&self) -> &[RGB<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_rgba(&self) -> &[RGBA<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_argb(&self) -> &[ARGB<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_rgb_mut(&mut self) -> &mut [RGB<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_rgba_mut(&mut self) -> &mut [RGBA<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_argb_mut(&mut self) -> &mut [ARGB<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_gray(&self) -> &[Gray<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_gray_alpha(&self) -> &[GrayAlpha<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_gray_mut(&mut self) -> &mut [Gray<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_gray_alpha_mut(&mut self) -> &mut [GrayAlpha<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_bgr(&self) -> &[BGR<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_abgr(&self) -> &[ABGR<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_bgra(&self) -> &[BGRA<T>] {
        unsafe { from_items_to_struct(self) }
    }

    #[inline]
    fn as_bgr_mut(&mut self) -> &mut [BGR<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_bgra_mut(&mut self) -> &mut [BGRA<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }

    #[inline]
    fn as_abgr_mut(&mut self) -> &mut [ABGR<T>] {
        unsafe { from_items_to_struct_mut(self) }
    }
}

#[inline(always)]
unsafe fn from_items_to_struct<F, T>(from: &[F]) -> &[T] {
    debug_assert_eq!(0, mem::size_of::<T>() % mem::size_of::<F>());
    let len = from.len() / (mem::size_of::<T>() / mem::size_of::<F>());
    slice::from_raw_parts(from.as_ptr().cast::<T>(), len)
}

#[inline(always)]
unsafe fn from_items_to_struct_mut<F, T>(from: &mut [F]) -> &mut [T] {
    debug_assert_eq!(0, mem::size_of::<T>() % mem::size_of::<F>());
    let len = from.len() / (mem::size_of::<T>() / mem::size_of::<F>());
    slice::from_raw_parts_mut(from.as_mut_ptr().cast::<T>(), len)
}

macro_rules! rgb_impl_from {
    ($typename:ident, $from:ty, $to:ty) => {
        impl From<$typename<$from>> for $typename<$to> {
            #[inline(always)]
            fn from(other: $typename<$from>) -> Self {
                other.map(core::convert::Into::into)
            }
        }
    };
}

rgb_impl_from! {RGB, u8,i16}
rgb_impl_from! {RGB, u8,i32}
rgb_impl_from! {RGB, u8,u16}
rgb_impl_from! {RGB, u8,u32}
rgb_impl_from! {RGB, u16,i32}
rgb_impl_from! {RGB, u16,u32}
rgb_impl_from! {RGB, u16,u64}

rgb_impl_from! {RGB, u8,f32}
rgb_impl_from! {RGB, u8,f64}
rgb_impl_from! {RGB, u16,f32}
rgb_impl_from! {RGB, u16,f64}

rgb_impl_from! {RGB, i16,f32}
rgb_impl_from! {RGB, i16,f64}

rgb_impl_from! {RGB, i32,f64}
rgb_impl_from! {RGB, f32,f64}

rgb_impl_from! {RGBA, u16,i32}
rgb_impl_from! {RGBA, u16,u32}
rgb_impl_from! {RGBA, u16,u64}

rgb_impl_from! {RGBA, u8,i16}
rgb_impl_from! {RGBA, u8,u16}
rgb_impl_from! {RGBA, u8,u32}
rgb_impl_from! {RGBA, u8,f32}
rgb_impl_from! {RGBA, u8,f64}
rgb_impl_from! {RGBA, u16,f32}
rgb_impl_from! {RGBA, u16,f64}

rgb_impl_from! {RGBA, i16,f32}
rgb_impl_from! {RGBA, i16,f64}

rgb_impl_from! {RGBA, i32,f64}
rgb_impl_from! {RGBA, f32,f64}

macro_rules! reorder_impl_from {
    (@rgb $t1:ident, $t2:ident) => {
        reorder_impl_from!(@once $t1, $t2, r, g, b);
        reorder_impl_from!(@once $t2, $t1, r, g, b);
    };
    (@rgba $t1:ident, $t2:ident) => {
        reorder_impl_from!(@once $t1, $t2, r, g, b, a);
        reorder_impl_from!(@once $t2, $t1, r, g, b, a);
    };
    (@once $t1:ident, $t2:ident, $($component:ident),+) => {
        impl<T> From<$t1<T>> for $t2<T> where T: ::core::clone::Clone {
            fn from(other: $t1<T>) -> Self {
                let $t1 { $($component),+ } = other;
                Self {
                    $($component),+
                }
            }
        }
    }
}

reorder_impl_from!(@rgba RGBA, ARGB);
reorder_impl_from!(@rgba ABGR, ARGB);
reorder_impl_from!(@rgba BGRA, ARGB);
reorder_impl_from!(@rgba BGRA, ABGR);

reorder_impl_from!(@rgb RGB, BGR);
reorder_impl_from!(@rgba BGRA, RGBA);
reorder_impl_from!(@rgba ABGR, RGBA);
reorder_impl_from!(@rgb RGB, GRB);

impl<T: Clone> From<Gray<T>> for RGB<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn from(other: Gray<T>) -> Self {
        Self {
            r: other.clone().value(),
            g: other.clone().value(),
            b: other.value(),
        }
    }
}

impl<T: Clone> From<Gray<T>> for RGBA<T, u8> {
    #[inline(always)]
    #[allow(deprecated)]
    fn from(other: Gray<T>) -> Self {
        Self {
            r: other.clone().value(),
            g: other.clone().value(),
            b: other.value(),
            a: 255,
        }
    }
}

impl<T: Clone, A: Clone> From<GrayAlpha<T, A>> for RGBA<T, A> {
    #[inline(always)]
    #[allow(deprecated)]
    fn from(other: GrayAlpha<T, A>) -> Self {
        Self {
            r: other.v.clone(),
            g: other.v.clone(),
            b: other.v.clone(),
            a: other.a.clone(),
        }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsRef<T> for Gray<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsRef<[T]> for RGB<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        #[allow(deprecated)]
        ComponentSlice::as_slice(self)
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsRef<[T; 3]> for RGB<T> {
    fn as_ref(&self) -> &[T; 3] {
        unsafe { &*(self as *const Self).cast() }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsRef<[T]> for RGBA<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        #[allow(deprecated)]
        ComponentSlice::as_slice(self)
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsRef<[T; 4]> for RGBA<T> {
    fn as_ref(&self) -> &[T; 4] {
        unsafe { &*(self as *const Self).cast() }
    }
}

impl<T> AsRef<[T; 4]> for ARGB<T> {
    fn as_ref(&self) -> &[T; 4] {
        unsafe { &*(self as *const Self).cast() }
    }
}

impl<T> AsRef<[T; 4]> for BGRA<T> {
    fn as_ref(&self) -> &[T; 4] {
        unsafe { &*(self as *const Self).cast() }
    }
}

impl<T> AsRef<[T; 4]> for ABGR<T> {
    fn as_ref(&self) -> &[T; 4] {
        unsafe { &*(self as *const Self).cast() }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsRef<T> for GrayAlpha<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn as_ref(&self) -> &T {
        &self.0
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsRef<[T; 2]> for GrayAlpha<T> {
    fn as_ref(&self) -> &[T; 2] {
        unsafe { &*(self as *const Self).cast() }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsMut<T> for Gray<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsMut<[T]> for RGB<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        #[allow(deprecated)]
        ComponentSlice::as_mut_slice(self)
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsMut<[T; 3]> for RGB<T> {
    fn as_mut(&mut self) -> &mut [T; 3] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsMut<[T]> for RGBA<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        #[allow(deprecated)]
        ComponentSlice::as_mut_slice(self)
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsMut<[T; 4]> for RGBA<T> {
    fn as_mut(&mut self) -> &mut [T; 4] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}

#[cfg(not(feature = "unstable-experimental"))]
impl<T> AsMut<T> for GrayAlpha<T> {
    #[inline(always)]
    #[allow(deprecated)]
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[cfg(feature = "unstable-experimental")]
impl<T> AsMut<[T; 2]> for GrayAlpha<T> {
    fn as_mut(&mut self) -> &mut [T; 2] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}
