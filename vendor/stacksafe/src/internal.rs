#![doc(hidden)]

pub use stacker;

#[cfg(debug_assertions)]
thread_local! {
    static PROTECTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[inline(always)]
pub fn is_protected() -> bool {
    #[cfg(debug_assertions)]
    {
        PROTECTED.with(|p| p.get())
    }

    #[cfg(not(debug_assertions))]
    {
        true
    }
}

#[inline(always)]
pub fn with_protected<R>(callback: impl FnOnce() -> R) -> impl FnOnce() -> R {
    move || {
        #[cfg(debug_assertions)]
        {
            let old = PROTECTED.with(|p| p.replace(true));
            let ret = callback();
            PROTECTED.with(|p| p.set(old));
            ret
        }

        #[cfg(not(debug_assertions))]
        {
            callback()
        }
    }
}
