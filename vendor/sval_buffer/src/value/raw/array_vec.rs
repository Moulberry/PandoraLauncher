/*!
Fixed-capacity, stack-allocated storage for no-std builds.
*/

use crate::{
    std::{
        fmt, mem,
        ops::{Deref, DerefMut},
    },
    Error,
};

use super::{RawStorage, RawStorageMut, PARTS_CAP};

pub(crate) struct ArrayVec<T, const N: usize> {
    buf: [mem::MaybeUninit<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> Clone for ArrayVec<T, N> {
    fn clone(&self) -> Self {
        // A bitwise copy is a correct clone for `Copy` elements. It also
        // avoids typed reads of the elements themselves: a byte buffer
        // holding encoded parts has uninitialized holes (padding and
        // inactive enum variant bytes inside part payloads), which a
        // bitwise copy carries over without reading
        ArrayVec {
            buf: self.buf,
            len: self.len,
        }
    }
}

impl<T, const N: usize> Default for ArrayVec<T, N> {
    fn default() -> Self {
        ArrayVec {
            // SAFETY: An array of uninitialized values is valid
            buf: unsafe { mem::MaybeUninit::<[mem::MaybeUninit<T>; N]>::uninit().assume_init() },
            len: 0,
        }
    }
}

impl<T, const N: usize> Drop for ArrayVec<T, N> {
    fn drop(&mut self) {
        // SAFETY: Values up to `self.len` are initialized
        unsafe {
            crate::std::ptr::drop_in_place::<[T]>(&mut **self as *mut [T]);
        }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for ArrayVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T, const N: usize> Deref for ArrayVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        let buf = &self.buf[..self.len];

        // SAFETY: Values up to `self.len` are initialized
        unsafe { &*(buf as *const [mem::MaybeUninit<T>] as *const [T]) }
    }
}

impl<T, const N: usize> DerefMut for ArrayVec<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let buf = &mut self.buf[..self.len];

        // SAFETY: Values up to `self.len` are initialized
        unsafe { &mut *(buf as *mut [mem::MaybeUninit<T>] as *mut [T]) }
    }
}

impl<T, const N: usize> ArrayVec<T, N> {
    pub(crate) fn push(&mut self, value: T) -> Result<(), Error> {
        if self.len == N {
            return Err(Error::no_alloc("vec push"));
        }

        mem::MaybeUninit::write(&mut self.buf[self.len], value);
        self.len += 1;

        Ok(())
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        match self.len.checked_sub(1) {
            Some(i) => {
                self.len = i;

                // SAFETY: The value at `i` is initialized and being moved out of
                Some(unsafe { mem::MaybeUninit::assume_init_read(&self.buf[i]) })
            }
            None => None,
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Default::default()
    }
}

impl<const N: usize> ArrayVec<u8, N> {
    // A pointer to the start of the backing buffer.
    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buf.as_mut_ptr() as *mut u8
    }

    pub(crate) fn extend_from_slice(&mut self, v: &[u8]) -> Result<(), Error> {
        if self.len + v.len() > N {
            return Err(Error::no_alloc("value part"));
        }

        for (dst, src) in self.buf[self.len..].iter_mut().zip(v) {
            dst.write(*src);
        }

        self.len += v.len();

        Ok(())
    }

    // SAFETY: The caller must ensure `n` bytes past the end were written
    pub(crate) unsafe fn add_len(&mut self, n: usize) {
        debug_assert!(
            self.len + n <= N,
            "attempt to commit bytes past the reserved capacity"
        );

        self.len += n;
    }
}

impl RawStorage for ArrayVec<u8, PARTS_CAP> {
    #[inline]
    fn as_slice(&self) -> &[u8] {
        self
    }

    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self
    }
}

impl RawStorageMut for ArrayVec<u8, PARTS_CAP> {
    #[inline]
    fn reserve(&mut self, n: usize) -> Result<(), Error> {
        if self.len() + n > PARTS_CAP {
            Err(Error::no_alloc("value part"))
        } else {
            Ok(())
        }
    }

    #[inline]
    fn extend_from_slice(&mut self, v: &[u8]) -> Result<(), Error> {
        ArrayVec::extend_from_slice(self, v)
    }

    #[inline]
    fn capacity(&self) -> usize {
        PARTS_CAP
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        ArrayVec::as_mut_ptr(self)
    }

    #[inline]
    unsafe fn advance_len(&mut self, n: usize) {
        // SAFETY: Upheld by this function's contract
        unsafe { self.add_len(n) }
    }

    #[inline]
    fn clear(&mut self) {
        ArrayVec::clear(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::rc::Rc;

    #[test]
    fn push_pop() {
        let mut vec = ArrayVec::<_, 2>::default();

        assert!(vec.pop().is_none());

        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert!(vec.push(3).is_err());

        assert_eq!(2, vec.pop().unwrap());
        assert_eq!(1, vec.pop().unwrap());
        assert!(vec.pop().is_none());

        assert!(vec.push(1).is_ok());

        assert_eq!(1, vec.pop().unwrap());
        assert!(vec.pop().is_none());
    }

    #[test]
    fn destructors() {
        let mut vec = ArrayVec::<_, 5>::default();

        let a = Rc::new(1);
        let b = Rc::new(2);

        vec.push(a.clone()).unwrap();
        vec.push(b.clone()).unwrap();

        assert_eq!(2, Rc::strong_count(&a));
        assert_eq!(2, Rc::strong_count(&b));

        drop(vec);

        assert_eq!(1, Rc::strong_count(&a));
        assert_eq!(1, Rc::strong_count(&b));
    }
}
