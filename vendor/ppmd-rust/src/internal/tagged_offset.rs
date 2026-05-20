use std::ptr::NonNull;

#[cfg(feature = "unstable-tagged-offsets")]
const TAG_MASK: u32 = 0xE0000000;
#[cfg(feature = "unstable-tagged-offsets")]
const OFFSET_MASK: u32 = 0x1FFFFFFF;
const TAG_NULL: u32 = 0;
#[allow(dead_code)]
pub(crate) const TAG_BYTES: u32 = 1 << 29;
#[allow(dead_code)]
pub(crate) const TAG_NODE: u32 = 2 << 29;
#[allow(dead_code)]
pub(crate) const TAG_STATE: u32 = 3 << 29;
#[allow(dead_code)]
pub(crate) const TAG_CONTEXT: u32 = 4 << 29;

#[allow(dead_code)]
pub(crate) trait Pointee {
    const TAG: u32;
}

impl Pointee for u8 {
    const TAG: u32 = TAG_BYTES;
}

pub(crate) trait MemoryAllocator {
    fn base_memory_ptr(&self) -> NonNull<u8>;
    #[cfg(not(feature = "unstable-tagged-offsets"))]
    fn units_start(&self) -> NonNull<u8>;
    #[cfg(feature = "unstable-tagged-offsets")]
    fn size(&self) -> u32;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[repr(transparent)]
pub(crate) struct TaggedOffset(u32);

#[cfg(feature = "unstable-tagged-offsets")]
impl TaggedOffset {
    pub(crate) const fn null() -> TaggedOffset {
        TaggedOffset(TAG_NULL)
    }

    #[inline(always)]
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub(crate) const fn from_bytes_offset(raw: u32) -> Self {
        TaggedOffset::from_raw((raw & OFFSET_MASK) | TAG_BYTES)
    }

    #[inline(always)]
    pub(crate) unsafe fn from_ptr<T: Pointee, A: MemoryAllocator>(
        allocator: &A,
        ptr: NonNull<T>,
    ) -> TaggedOffset {
        let offset = ptr.cast().offset_from(allocator.base_memory_ptr());
        let offset = u32::try_from(offset).expect("Failed to convert ptr to offset");
        let val = (offset & OFFSET_MASK) | T::TAG;
        Self(val)
    }

    #[inline(always)]
    pub(crate) fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    #[inline(always)]
    pub(crate) fn is_not_null(&self) -> bool {
        self.0 != TAG_NULL
    }

    #[inline(always)]
    pub(crate) fn get_offset(&self) -> u32 {
        self.0 & OFFSET_MASK
    }

    #[inline(always)]
    pub(crate) fn as_raw(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub(crate) unsafe fn as_ptr<T: Pointee, A: MemoryAllocator>(
        &self,
        allocator: &A,
    ) -> NonNull<T> {
        let offset = self.get_offset();

        assert_eq!(self.0 & TAG_MASK, T::TAG, "Mismatched pointer type tag");
        assert!(offset < allocator.size(), "Out of bound access");

        allocator.base_memory_ptr().offset(offset as isize).cast()
    }

    pub(crate) fn is_real_context<A: MemoryAllocator>(&self, _allocator: &A) -> bool {
        (self.0 & TAG_MASK) == TAG_CONTEXT
    }
}

#[cfg(not(feature = "unstable-tagged-offsets"))]
impl TaggedOffset {
    pub(crate) const fn null() -> TaggedOffset {
        TaggedOffset(TAG_NULL)
    }

    #[inline(always)]
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub(crate) const fn from_bytes_offset(raw: u32) -> Self {
        TaggedOffset::from_raw(raw)
    }

    #[inline(always)]
    pub(crate) unsafe fn from_ptr<T: Pointee, A: MemoryAllocator>(
        allocator: &A,
        ptr: NonNull<T>,
    ) -> TaggedOffset {
        let offset = ptr.cast().offset_from(allocator.base_memory_ptr());
        let offset = u32::try_from(offset).expect("Failed to convert ptr to offset");
        Self(offset)
    }

    #[inline(always)]
    pub(crate) fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    #[inline(always)]
    pub(crate) fn is_not_null(&self) -> bool {
        self.0 != TAG_NULL
    }

    #[inline(always)]
    pub(crate) fn get_offset(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub(crate) fn as_raw(&self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub(crate) unsafe fn as_ptr<T: Pointee, A: MemoryAllocator>(
        &self,
        allocator: &A,
    ) -> NonNull<T> {
        let offset = self.get_offset();
        allocator.base_memory_ptr().offset(offset as isize).cast()
    }

    pub(crate) unsafe fn is_real_context<A: MemoryAllocator>(&self, allocator: &A) -> bool {
        // A "real" context must be in the unit area and not in the text buffer area.
        let ptr = allocator
            .base_memory_ptr()
            .offset(self.get_offset() as isize)
            .cast();
        ptr >= allocator.units_start()
    }
}
