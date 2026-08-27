use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::num::NonZeroUsize;
use core::slice;

#[cfg(test)]
use alloc::vec;

/// Rows of the image. Call `Img.rows()` to create it.
///
/// Each element is a slice `width` pixels wide. Ignores padding, if there's any.
#[derive(Debug)]
#[must_use]
pub struct RowsIter<'a, T> {
    pub(crate) inner: slice::Chunks<'a, T>,
    pub(crate) width: usize,
}

impl<'a, T: 'a> Iterator for RowsIter<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(s) => {
                // guaranteed during creation of chunks iterator
                debug_assert!(s.len() >= self.width);
                unsafe {
                    Some(s.get_unchecked(0..self.width))
                }
            },
            None => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.inner.nth(n) {
            Some(s) => {
                // guaranteed during creation of chunks iterator
                debug_assert!(s.len() >= self.width);
                unsafe {
                    Some(s.get_unchecked(0..self.width))
                }
            },
            None => None,
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<T> ExactSizeIterator for RowsIter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T> FusedIterator for RowsIter<'_, T> {}

impl<'a, T: 'a> DoubleEndedIterator for RowsIter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.inner.next_back() {
            Some(s) => {
                // guaranteed during creation of chunks iterator
                debug_assert!(s.len() >= self.width);
                unsafe {
                    Some(s.get_unchecked(0..self.width))
                }
            },
            None => None,
        }
    }
}

/// Rows of the image. Call `Img.rows_mut()` to create it.
///
/// Each element is a slice `width` pixels wide. Ignores padding, if there's any.
#[derive(Debug)]
#[must_use]
pub struct RowsIterMut<'a, T> {
    pub(crate) width: usize,
    pub(crate) inner: slice::ChunksMut<'a, T>,
}

impl<'a, T: 'a> Iterator for RowsIterMut<'a, T> {
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(s) => Some(&mut s[0..self.width]),
            None => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.inner.nth(n) {
            Some(s) => Some(&mut s[0..self.width]),
            None => None,
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }
}

impl<T> ExactSizeIterator for RowsIterMut<'_, T> {}
impl<T> FusedIterator for RowsIterMut<'_, T> {}

impl<'a, T: 'a> DoubleEndedIterator for RowsIterMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.inner.next_back() {
            Some(s) => Some(&mut s[0..self.width]),
            None => None,
        }
    }
}

/// Iterates over pixels in the (sub)image. Call `Img.pixels()` to create it.
///
/// Ignores padding, if there's any.
#[must_use]
pub struct PixelsIter<'a, T: Copy> {
    inner: PixelsRefIter<'a, T>,
}

impl<'a, T: Copy + 'a> PixelsIter<'a, T> {
    #[inline(always)]
    #[track_caller]
    pub(crate) fn new(img: super::ImgRef<'a, T>) -> Self {
        Self {
            inner: PixelsRefIter::new(img)
        }
    }
}

impl<'a, T: Copy + 'a> Iterator for PixelsIter<'a, T> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().copied()
    }
}

impl<T: Copy> ExactSizeIterator for PixelsIter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T: Copy> FusedIterator for PixelsIter<'_, T> {}

/// Iterates over pixels in the (sub)image. Call `Img.pixels_ref()` to create it.
///
/// Ignores padding, if there's any.
#[derive(Debug)]
#[must_use]
pub struct PixelsRefIter<'a, T> {
    current: *const T,
    current_line_end: *const T,
    rows_left: usize,
    width: NonZeroUsize,
    pad: usize,
    _dat: PhantomData<&'a [T]>,
}

unsafe impl<T> Send for PixelsRefIter<'_, T> where T: Sync {}
unsafe impl<T> Sync for PixelsRefIter<'_, T> where T: Sync {}

impl<'a, T: 'a> PixelsRefIter<'a, T> {
    #[inline]
    #[track_caller]
    pub(crate) fn new(img: super::ImgRef<'a, T>) -> Self {
        let buf = img.valid_buf();
        let height = img.height();
        match NonZeroUsize::new(img.width()) {
            Some(width) if height > 0 => {
                let stride = img.stride();
                let pad = stride - width.get();
                Self {
                    current: buf.as_ptr(),
                    current_line_end: buf[width.get()..].as_ptr(),
                    width,
                    rows_left: height - 1,
                    pad,
                    _dat: PhantomData,
                }
            },
            _ => {
                Self {
                    current: buf.as_ptr(),
                    current_line_end: buf.as_ptr(),
                    width: NonZeroUsize::new(1).unwrap(),
                    rows_left: 0,
                    pad: 0,
                    _dat: PhantomData,
                }
            }
        }
    }
}

impl<'a, T: 'a> Iterator for PixelsRefIter<'a, T> {
    type Item = &'a T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.current >= self.current_line_end {
                if self.rows_left == 0 {
                    return None;
                }
                self.rows_left -= 1;
                self.current = self.current_line_end.add(self.pad);
                self.current_line_end = self.current.add(self.width.get());
            }
            let px = &*self.current;
            self.current = self.current.add(1);
            Some(px)
        }
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let this_line = unsafe {
            self.current_line_end.offset_from(self.current)
        };
        debug_assert!(this_line >= 0);
        let len = this_line as usize + self.rows_left * self.width.get();
        (len, Some(len))
    }
}

impl<T: Copy> ExactSizeIterator for PixelsRefIter<'_, T> {
}

impl<T> FusedIterator for PixelsRefIter<'_, T> {}

/// Iterates over pixels in the (sub)image. Call `Img.pixels_mut()` to create it.
///
/// Ignores padding, if there's any.
#[derive(Debug)]
#[must_use]
pub struct PixelsIterMut<'a, T> {
    current: *mut T,
    current_line_end: *mut T,
    rows_left: usize,
    width: NonZeroUsize,
    pad: usize,
    _dat: PhantomData<&'a mut [T]>,
}

unsafe impl<T> Send for PixelsIterMut<'_, T> where T: Send {}
unsafe impl<T> Sync for PixelsIterMut<'_, T> where T: Sync {}

impl<'a, T: 'a> PixelsIterMut<'a, T> {
    #[inline]
    #[track_caller]
    pub(crate) fn new(mut img: super::ImgRefMut<'a, T>) -> Self {
        let width = img.width();
        let height = img.height();
        let stride = img.stride();
        let buf = img.valid_buf_mut();
        let ptr = buf.as_mut_ptr();
        match NonZeroUsize::new(width) {
            Some(width) if height > 0 => {
                Self {
                    current: ptr,
                    current_line_end: unsafe { ptr.add(width.get()) },
                    width,
                    rows_left: height - 1,
                    pad: stride - width.get(),
                    _dat: PhantomData,
                }
            },
            _ => {
                Self {
                    current: ptr,
                    current_line_end: ptr,
                    width: NonZeroUsize::new(1).unwrap(),
                    rows_left: 0,
                    pad: 0,
                    _dat: PhantomData,
                }
            }
        }
    }
}

impl<'a, T: 'a> Iterator for PixelsIterMut<'a, T> {
    type Item = &'a mut T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.current >= self.current_line_end {
                if self.rows_left == 0 {
                    return None;
                }
                self.rows_left -= 1;
                self.current = self.current_line_end.add(self.pad);
                self.current_line_end = self.current.add(self.width.get());
            }
            let px = &mut *self.current;
            self.current = self.current.add(1);
            Some(px)
        }
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let this_line = unsafe {
            self.current_line_end.offset_from(self.current)
        };
        debug_assert!(this_line >= 0);
        let len = this_line as usize + self.rows_left * self.width.get();
        (len, Some(len))
    }
}

impl<T: Copy> ExactSizeIterator for PixelsIterMut<'_, T> {
}

impl<T> FusedIterator for PixelsIterMut<'_, T> {}

#[test]
fn iter() {
    let img = super::Img::new(vec![1u8, 2], 1, 2);
    let mut it = img.pixels();
    assert_eq!(Some(1), it.next());
    assert_eq!(Some(2), it.next());
    assert_eq!(None, it.next());

    let buf = [1u8; (16 + 3) * (8 + 1)];
    for width in 1..16 {
        for height in 1..8 {
            for pad in 0..3 {
                let stride = width + pad;
                let img = super::Img::new_stride(&buf[..stride * height + stride - width], width, height, stride);
                assert_eq!(width * height, img.pixels().map(|a| a as usize).sum(), "{width}x{height}");
                assert_eq!(width * height, img.pixels().count(), "{width}x{height}");
                assert_eq!(height, img.rows().count());

                let mut iter1 = img.pixels();
                let mut left = width * height;
                while let Some(_px) = iter1.next() {
                    left -= 1;
                    assert_eq!(left, iter1.len());
                }
                assert_eq!(0, iter1.len());
                assert_eq!(0, left);
                iter1.next();
                assert_eq!(0, iter1.len());

                let mut iter2 = img.rows();
                match iter2.next() {
                    Some(_) => {
                        assert_eq!(height - 1, iter2.size_hint().0);
                        assert_eq!(height - 1, iter2.filter(|_| true).count());
                    },
                    None => {
                        assert_eq!(height, 0);
                    },
                }
            }
        }
    }
}

#[test]
#[should_panic(expected = "Invalid ImgRef params")]
fn rows_iter_len_overflow_can_create_oob_slice() {
    // `ImgRef::valid_min_len()` computes `stride * height + width - stride`
    // using checked arithmetic. It must panic on overflow instead of accepting
    // a one-element buffer for a 2x2 image with an impossible stride and later
    // reaching `RowsIter::next()`'s unsafe `get_unchecked(0..2)`.
    let buf = [0u8; 1];
    let img = super::Img::new_stride(&buf[..], 2, 2, usize::MAX);

    let _ = img.rows().next();
}

#[test]
#[should_panic(expected = "Invalid ImgRef params")]
fn pixels_ref_len_overflow_can_walk_oob() {
    // The same checked length calculation must panic on overflow for a 1x3
    // image, rather than letting `PixelsRefIter` start from a one-element slice
    // and later move the raw pointer far outside the allocation.
    let buf = [0u8; 1];
    let img = super::Img::new_stride(&buf[..], 1, 3, usize::MAX / 2 + 1);
    let mut pixels = img.pixels_ref();

    let _ = pixels.next();
    let _ = pixels.next();
}

#[test]
fn pixels_ref_iter_send_requires_sync_pixels() {
    // `PixelsRefIter<'_, T>` yields `&T`, so sending it to another thread is
    // only sound when `T: Sync`. `Cell<u32>` is `Send` but not `Sync`; if the
    // iterator were `Send` for `T: Send`, safe code could create a data race by
    // sending the iterator to another thread while retaining local shared
    // access to the same cell.
    use core::cell::Cell;

    macro_rules! assert_not_impl_any {
        ($x:ty: $($t:path),+ $(,)?) => {
            const _: fn() = || {
                trait AmbiguousIfImpl<A> { fn some_item() {} }
                impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
                impl<T: ?Sized $(+ $t)+> AmbiguousIfImpl<u8> for T {}
                <$x as AmbiguousIfImpl<_>>::some_item()
            };
        };
    }

    fn assert_send<T: Send>() {}

    assert_send::<PixelsRefIter<'static, u32>>();
    assert_not_impl_any!(PixelsRefIter<'static, Cell<u32>>: Send);
}

#[test]
#[should_panic(expected = "Invalid ImgRef params")]
fn pixels_mut_len_overflow_can_create_oob_line_end() {
    // `PixelsIterMut::new()` computes `ptr.add(width)` for the first row's
    // line end. Checked validation must reject this wrapped 2x2 image before
    // creating a one-element mutable slice where that pointer is out of bounds.
    let mut img = super::Img::new_stride(vec![0u8; 1], 2, 2, usize::MAX);

    let _ = img.pixels_mut();
}

#[test]
#[should_panic(expected = "Invalid ImgRef params")]
fn pixels_mut_len_overflow_can_walk_oob() {
    // Mutable pixel iteration has the same invariant: checked validation must
    // reject this wrapped 1x3 image before the second row would require raw
    // pointer arithmetic far outside the one-element allocation.
    let mut buf = [0u8; 1];
    let mut img = super::Img::new_stride(&mut buf[..], 1, 3, usize::MAX / 2 + 1);
    let mut pixels = img.pixels_mut();

    let _ = pixels.next();
    let _ = pixels.next();
}
