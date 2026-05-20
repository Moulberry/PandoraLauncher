use crate::alt::ARGB;
use crate::alt::{BGR, BGRA};
use crate::{RGB, RGBA};

impl<T: Copy> From<[T; 3]> for RGB<T> {
    #[inline(always)]
    fn from(other: [T; 3]) -> Self {
        Self {
            r: other[0],
            g: other[1],
            b: other[2],
        }
    }
}

impl<T> From<RGB<T>> for [T; 3] {
    #[inline(always)]
    fn from(value: RGB<T>) -> Self {
        [value.r, value.g, value.b]
    }
}

impl<T: Copy> From<[T; 4]> for RGBA<T> {
    #[inline(always)]
    fn from(other: [T; 4]) -> Self {
        Self {
            r: other[0],
            g: other[1],
            b: other[2],
            a: other[3],
        }
    }
}

impl<T> From<RGBA<T>> for [T; 4] {
    #[inline(always)]
    fn from(value: RGBA<T>) -> Self {
        [value.r, value.g, value.b, value.a]
    }
}

impl<T: Copy> From<[T; 4]> for ARGB<T> {
    #[inline(always)]
    fn from(other: [T; 4]) -> Self {
        Self {
            a: other[0],
            r: other[1],
            g: other[2],
            b: other[3],
        }
    }
}

impl<T> Into<[T; 4]> for ARGB<T> {
    #[inline(always)]
    fn into(self) -> [T; 4] {
        [self.a, self.r, self.g, self.b]
    }
}

impl<T: Copy> From<[T; 3]> for BGR<T> {
    #[inline(always)]
    fn from(other: [T; 3]) -> Self {
        Self {
            b: other[0],
            g: other[1],
            r: other[2],
        }
    }
}

impl<T> From<BGR<T>> for [T; 3] {
    #[inline(always)]
    fn from(value: BGR<T>) -> Self {
        [value.b, value.g, value.r]
    }
}

impl<T: Copy> From<[T; 4]> for BGRA<T> {
    #[inline(always)]
    fn from(other: [T; 4]) -> Self {
        Self {
            b: other[0],
            g: other[1],
            r: other[2],
            a: other[3],
        }
    }
}

impl<T> From<BGRA<T>> for [T; 4] {
    #[inline(always)]
    fn from(value: BGRA<T>) -> Self {
        [value.b, value.g, value.r, value.a]
    }
}
