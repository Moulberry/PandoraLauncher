use std::alloc::{alloc, handle_alloc_error, Layout};
use std::mem::align_of;

use super::c::{MAXIMUM_REPARSE_DATA_BUFFER_SIZE, REPARSE_DATA_BUFFER};

type MaybeU8 = std::mem::MaybeUninit<u8>;

#[repr(align(4))]
pub struct BytesAsReparseDataBuffer {
    value: Box<[MaybeU8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]>,
}

// Asserts that pointers of `BytesAsReparseDataBuffer` can be casted to
// `REPARSE_DATA_BUFFER`.
const _: () = {
    let a = align_of::<BytesAsReparseDataBuffer>();
    let b = align_of::<REPARSE_DATA_BUFFER>();
    assert!((a % b) == 0);
};

impl BytesAsReparseDataBuffer {
    // MSRV(1.82): Use `Box::new_uninit_slice` instead.
    pub fn new() -> Self {
        type Raw = [MaybeU8; MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize];
        const LAYOUT: Layout = Layout::new::<Raw>();
        let boxed = unsafe {
            let ptr = alloc(LAYOUT).cast::<Raw>();
            if ptr.is_null() {
                handle_alloc_error(LAYOUT);
            }
            Box::from_raw(ptr)
        };
        Self { value: boxed }
    }

    pub fn as_mut_ptr(&mut self) -> *mut REPARSE_DATA_BUFFER {
        self.value.as_mut_ptr().cast::<REPARSE_DATA_BUFFER>()
    }

    pub unsafe fn assume_init(&mut self) -> &REPARSE_DATA_BUFFER {
        &*self.as_mut_ptr()
    }
}
