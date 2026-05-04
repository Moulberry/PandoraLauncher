use std::alloc::{GlobalAlloc, Layout, System};

// The world's dumbest allocator. Just keep bumping a pointer until we run out
// of memory, in which case we abort. StringCache is responsible for creating
// a new allocator when that's about to happen.
// This is now bumping downward rather than up, which simplifies the allocate()
// method and gives a small (5-7%) performance improvement in multithreaded
// benchmarks
// See https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
pub(crate) struct LeakyBumpAlloc {
    layout: Layout,
    start: *mut u8,
    end: *mut u8,
    ptr: *mut u8,
}

impl LeakyBumpAlloc {
    pub fn new(capacity: usize, alignment: usize) -> LeakyBumpAlloc {
        let layout = Layout::from_size_align(capacity, alignment).unwrap();
        let start = unsafe { System.alloc(layout) };
        if start.is_null() {
            panic!("oom");
        }
        let end = unsafe { start.add(layout.size()) };
        let ptr = end;
        LeakyBumpAlloc {
            layout,
            start,
            end,
            ptr,
        }
    }

    #[doc(hidden)]
    // used for resetting the cache between benchmark runs. DO NOT CALL THIS.
    pub unsafe fn clear(&mut self) {
        System.dealloc(self.start, self.layout);
    }

    // Allocates a new chunk. Aborts if out of memory.
    pub unsafe fn allocate(&mut self, num_bytes: usize) -> *mut u8 {
        // Our new ptr will be offset down the heap by num_bytes bytes.
        let ptr = self.ptr as usize;
        let new_ptr = ptr.checked_sub(num_bytes).expect("ptr sub overflowed");
        // Round down to alignment.
        let new_ptr = new_ptr & !(self.layout.align() - 1);
        // Check we have enough capacity.
        let start = self.start as usize;
        if new_ptr < start {
            eprintln!(
                "Allocator asked to bump to {} bytes with a capacity of {}",
                self.end as usize - new_ptr,
                self.capacity()
            );
            // We have to abort here rather than panic or the mutex may
            // deadlock.
            std::process::abort();
        }

        self.ptr = self.ptr.sub(ptr - new_ptr);
        self.ptr
    }

    pub fn allocated(&self) -> usize {
        self.end as usize - self.ptr as usize
    }

    pub fn capacity(&self) -> usize {
        self.layout.size()
    }

    pub(crate) fn end(&self) -> *const u8 {
        self.end
    }

    pub(crate) fn ptr(&self) -> *const u8 {
        self.ptr
    }
}
