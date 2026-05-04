use alloc::boxed::Box;
#[cfg(any(debug_assertions, miri))]
use core::any::{self, TypeId};
use core::mem;

pub(crate) struct ErasedValue {
    ptr: *mut (),
    drop: unsafe fn(*mut ()),
    #[cfg(any(debug_assertions, miri))]
    type_id: TypeId,
    #[cfg(any(debug_assertions, miri))]
    type_name: &'static str,
}

impl ErasedValue {
    pub(crate) unsafe fn new<T>(value: T) -> Self {
        ErasedValue {
            ptr: Box::into_raw(Box::new(value)).cast(),
            drop: {
                unsafe fn drop<T>(ptr: *mut ()) {
                    let _ = unsafe { Box::from_raw(ptr.cast::<T>()) };
                }
                drop::<T>
            },
            #[cfg(any(debug_assertions, miri))]
            type_id: typeid::of::<T>(),
            #[cfg(any(debug_assertions, miri))]
            type_name: any::type_name::<T>(),
        }
    }

    pub(crate) unsafe fn take<T>(self) -> T {
        #[cfg(any(debug_assertions, miri))]
        assert_eq!(
            self.type_id,
            typeid::of::<T>(),
            "ErasedValue mismatch: {} vs {}",
            self.type_name,
            any::type_name::<T>(),
        );

        let b = unsafe { Box::from_raw(self.ptr.cast::<T>()) };
        mem::forget(self);
        *b
    }
}

impl Drop for ErasedValue {
    fn drop(&mut self) {
        unsafe { (self.drop)(self.ptr) }
    }
}
