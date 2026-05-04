use std::{alloc::Layout, borrow::{Borrow, Cow}, hash::Hash, ops::Deref, ptr::NonNull, sync::atomic::{AtomicUsize, Ordering}};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize, de::Visitor};

const MAX_REFS: usize = isize::MAX as usize;

#[derive(Debug)]
#[repr(C)]
struct UniqueBytesInner {
    count: AtomicUsize,
    data: [u8],
}

unsafe impl Send for UniqueBytesInner {}
unsafe impl Sync for UniqueBytesInner {}

impl UniqueBytesInner {
    fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct UniqueBytesPtr(NonNull<UniqueBytesInner>);

impl Hash for UniqueBytesPtr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the data, not the pointer
        self.inner().as_bytes().hash(state);
    }
}

impl Borrow<[u8]> for UniqueBytesPtr {
    fn borrow(&self) -> &[u8] {
        unsafe { self.0.as_ref().as_bytes() }
    }
}

unsafe impl Send for UniqueBytesPtr {}
unsafe impl Sync for UniqueBytesPtr {}

impl UniqueBytesPtr {
    fn create_for_slice(v: &[u8]) -> Self {
        unsafe {
            let len = v.len();

            let value_layout = Layout::array::<u8>(len).unwrap();
            let layout = Layout::new::<AtomicUsize>().extend(value_layout).unwrap().0.pad_to_align();

            let ptr = std::alloc::alloc(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            let inner = std::ptr::slice_from_raw_parts_mut(ptr, len) as *mut UniqueBytesInner;

            (&raw mut (*inner).count).write(AtomicUsize::new(1));
            std::ptr::copy_nonoverlapping(v.as_ptr(), (&raw mut (*inner).data) as *mut u8, v.len());

            Self(NonNull::new_unchecked(inner))
        }
    }

    fn inner(&self) -> &UniqueBytesInner {
        unsafe { self.0.as_ref() }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct UniqueBytes(UniqueBytesPtr);

impl Hash for UniqueBytes {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the pointer, not the data
        self.0.0.hash(state);
    }
}

impl Clone for UniqueBytes {
    fn clone(&self) -> Self {
        Self::clone_ptr_increase_refcnt(&self.0)
    }
}

impl Deref for UniqueBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0.inner().data
    }
}

impl From<&[u8]> for UniqueBytes {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

impl From<Vec<u8>> for UniqueBytes {
    fn from(value: Vec<u8>) -> Self {
        Self::new(&value)
    }
}

impl From<Cow<'_, [u8]>> for UniqueBytes {
    fn from(value: Cow<'_, [u8]>) -> Self {
        Self::new(value.as_ref())
    }
}

impl Drop for UniqueBytes {
    fn drop(&mut self) {
        if self.0.inner().count.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        std::sync::atomic::fence(Ordering::Acquire);

        let removed = UNIQUE.lock().remove(&self.0);
        debug_assert!(removed);

        unsafe {
            let bytes = self.0.0.as_ptr() as *mut [u8] as *mut u8;
            std::alloc::dealloc(bytes, Layout::for_value(self.0.0.as_ref()));
        }
    }
}

static UNIQUE: Lazy<Mutex<FxHashSet<UniqueBytesPtr>>> = Lazy::new(Default::default);

impl UniqueBytes {
    pub fn new(bytes: &[u8]) -> UniqueBytes {
        let mut set = UNIQUE.lock();
        if let Some(ptr) = set.get(bytes) {
            return Self::clone_ptr_increase_refcnt(ptr);
        }

        let ptr = UniqueBytesPtr::create_for_slice(bytes);
        set.insert(ptr.clone());
        Self(ptr)
    }

    fn clone_ptr_increase_refcnt(ptr: &UniqueBytesPtr) -> Self {
        let old_size = ptr.inner().count.fetch_add(1, Ordering::Relaxed);
        if old_size > MAX_REFS {
            std::process::abort();
        }
        Self(ptr.clone())
    }
}

impl Serialize for UniqueBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        serializer.serialize_bytes(&**self)
    }
}

impl <'de> Deserialize<'de> for UniqueBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        deserializer.deserialize_bytes(UniqueBytesVisitor)
    }
}

struct UniqueBytesVisitor;

impl <'de> Visitor<'de> for UniqueBytesVisitor {
    type Value = UniqueBytes;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("bytes")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>
    {
        let capacity = seq.size_hint().unwrap_or(0).max(0);
        let mut values = Vec::<u8>::with_capacity(capacity);

        while let Some(element) = seq.next_element()? {
            values.push(element);
        }

        Ok(UniqueBytes::new(&values))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error
    {
        Ok(UniqueBytes::new(v))
    }
}
