#[allow(deprecated)]
use super::pixel::ComponentSlice;
use super::pixel::{ComponentMap, ColorComponentMap};
#[cfg(feature = "as-bytes")]
use super::pixel::ComponentBytes;
use crate::alt::GRB;
use crate::alt::{BGR, BGRA};
use crate::{RGB, RGBA};
use core::fmt;

impl<T> BGR<T> {
    /// Convenience function for creating a new pixel
    /// Warning: The order of arguments is R,G,B
    #[deprecated(note = "This function has a misleading order of arguments. Use BGR{} literal instead")]
    pub const fn new(r: T, g: T, b: T) -> Self {
        Self { b, g, r }
    }
}

macro_rules! impl_rgb {
    ($RGB:ident) => {
        impl<T: Clone> $RGB<T> {
            /// Iterate over color components (R, G, and B)
            #[inline(always)]
            pub fn iter(&self) -> core::iter::Cloned<core::slice::Iter<'_, T>> {
                #[allow(deprecated)]
                ComponentSlice::as_slice(self).iter().cloned()
            }
        }

        impl<T: Copy, B> ComponentMap<$RGB<B>, T, B> for $RGB<T> {
            #[inline(always)]
            fn map<F>(&self, mut f: F) -> $RGB<B>
                where F: FnMut(T) -> B {
                $RGB {
                    r:f(self.r),
                    g:f(self.g),
                    b:f(self.b),
                }
            }
        }

        impl<T: Copy, B> ColorComponentMap<$RGB<B>, T, B> for $RGB<T> {
            #[inline(always)]
            fn map_colors<F>(&self, mut f: F) -> $RGB<B>
                where F: FnMut(T) -> B {
                $RGB {
                    r:f(self.r),
                    g:f(self.g),
                    b:f(self.b),
                }
            }
        }

        #[allow(deprecated)]
        impl<T> ComponentSlice<T> for $RGB<T> {
            #[inline(always)]
            fn as_slice(&self) -> &[T] {
                unsafe {
                    core::slice::from_raw_parts(self as *const Self as *const T, 3)
                }
            }

            #[inline(always)]
            fn as_mut_slice(&mut self) -> &mut [T] {
                unsafe {
                    core::slice::from_raw_parts_mut(self as *mut Self as *mut T, 3)
                }
            }
        }

        #[allow(deprecated)]
        impl<T> ComponentSlice<T> for [$RGB<T>] {
            #[inline]
            fn as_slice(&self) -> &[T] {
                unsafe {
                    core::slice::from_raw_parts(self.as_ptr() as *const _, self.len() * 3)
                }
            }

            #[inline]
            fn as_mut_slice(&mut self) -> &mut [T] {
                unsafe {
                    core::slice::from_raw_parts_mut(self.as_mut_ptr() as *mut _, self.len() * 3)
                }
            }
        }

        #[cfg(feature = "as-bytes")]
        impl<T: crate::Pod> ComponentBytes<T> for [$RGB<T>] {}
    };
}

macro_rules! impl_rgb_to_alpha {
    ($RGB:ident, $RGBA:ident) => {
        impl<T: Clone> $RGB<T> {
            /// Convenience function for converting to RGBA
            #[doc(hidden)]
            #[deprecated(note = "use .with_alpha(a) instead; this will become a getter in the future")]
            pub fn alpha(&self, a: T) -> $RGBA<T> {
                self.with_alpha(a)
            }

            /// Convenience function for converting to RGBA
            #[inline(always)]
            #[doc(alias = "alpha")]
            pub fn with_alpha(&self, a: T) -> $RGBA<T> {
                $RGBA {
                    r: self.r.clone(),
                    g: self.g.clone(),
                    b: self.b.clone(),
                    a,
                }
            }

            /// Convenience function for converting to RGBA with alpha channel of a different type than type of the pixels
            #[inline(never)]
            #[deprecated(note = "use .with_alpha(a) instead")]
            pub fn new_alpha<A>(&self, a: A) -> $RGBA<T, A> {
                $RGBA {
                    r: self.r.clone(),
                    g: self.g.clone(),
                    b: self.b.clone(),
                    a,
                }
            }
        }
    };
}

impl<T> core::iter::FromIterator<T> for RGB<T> {
    /// Takes exactly 3 elements from the iterator and creates a new instance.
    /// Panics if there are fewer elements in the iterator.
    #[inline(always)]
    fn from_iter<I: IntoIterator<Item = T>>(into_iter: I) -> Self {
        let mut iter = into_iter.into_iter();
        Self {
            r: iter.next().unwrap(),
            g: iter.next().unwrap(),
            b: iter.next().unwrap(),
        }
    }
}

impl_rgb! {RGB}
impl_rgb_to_alpha! {RGB, RGBA}
impl_rgb! {BGR}
impl_rgb_to_alpha! {BGR, BGRA}
impl_rgb! {GRB}

impl<T: fmt::Display> fmt::Display for RGB<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgb({},{},{})", self.r, self.g, self.b)
    }
}

impl<T: fmt::UpperHex> fmt::UpperHex for RGB<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RGB {{ #{:02X}{:02X}{:02X} }}", self.r, self.g, self.b)
    }
}

impl<T: fmt::LowerHex> fmt::LowerHex for RGB<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RGB {{ #{:02x}{:02x}{:02x} }}", self.r, self.g, self.b)
    }
}

impl<T: fmt::Display> fmt::Display for BGR<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bgr({},{},{})", self.b, self.g, self.r)
    }
}

impl<T: fmt::UpperHex> fmt::UpperHex for BGR<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BGR {{ #{:02X}{:02X}{:02X} }}", self.b, self.g, self.r)
    }
}

impl<T: fmt::LowerHex> fmt::LowerHex for BGR<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BGR {{ #{:02x}{:02x}{:02x} }}", self.b, self.g, self.r)
    }
}
