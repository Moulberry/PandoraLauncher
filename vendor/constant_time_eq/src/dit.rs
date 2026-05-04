//! Runs code with the hardware DIT feature enabled when possible.
//!
//! With the "std" feature, detects at compilation time and runtime whether FEAT_DIT is available,
//! and uses it (together with FEAT_SB when available) to enable the data independent timing mode
//! of the processor.
//!
//! Without the "std" feature, this detection is done at compilation time only, which is enough for
//! some targets like aarch64-apple-darwin which is known to always have these features.

use core::arch::asm;

/// Describes whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
#[repr(u8)]
#[derive(Clone, Copy)]
enum Features {
    // Unknown = 0,
    #[allow(dead_code)]
    Neither = 1,
    #[allow(dead_code)]
    DitOnly = 2,
    DitSb = 3,
}

#[cfg(not(all(target_feature = "dit", target_feature = "sb")))]
mod detect {
    use super::Features;
    use core::mem::transmute;
    use core::sync::atomic::{AtomicU8, Ordering};

    static FEATURES: AtomicU8 = AtomicU8::new(0);

    /// Determines whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
    #[inline]
    pub fn get_aarch64_dit_sb_features() -> Features {
        let features = FEATURES.load(Ordering::Relaxed);
        if features > 0 {
            // SAFETY: a non-zero value is a valid discriminant from Features
            unsafe { transmute::<u8, Features>(features) }
        } else {
            detect_aarch64_dit_sb_features()
        }
    }

    /// Records whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
    ///
    /// # Safety
    ///
    /// Either parameter must not be set to true if the corresponding feature
    /// is not implemented.
    pub unsafe fn set_aarch64_dit_sb_features(dit: bool, sb: bool) -> Features {
        let dit = dit || cfg!(target_feature = "dit");
        let sb = sb || cfg!(target_feature = "sb");
        let features = match (dit, sb) {
            (true, true) => Features::DitSb,
            (true, false) => Features::DitOnly,
            _ => Features::Neither,
        };
        let _ = FEATURES.compare_exchange(0, features as u8, Ordering::Relaxed, Ordering::Relaxed);
        features
    }

    /// Detects whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
    #[cfg(feature = "std")]
    #[cold]
    fn detect_aarch64_dit_sb_features() -> Features {
        use std::arch::is_aarch64_feature_detected;
        // SAFETY: each parameter is true only if the feature is implemented
        unsafe {
            set_aarch64_dit_sb_features(
                is_aarch64_feature_detected!("dit"),
                is_aarch64_feature_detected!("sb"),
            )
        }
    }

    /// Detects whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
    #[cfg(not(feature = "std"))]
    #[cold]
    fn detect_aarch64_dit_sb_features() -> Features {
        // It might or might not be possible to read the system registers
        // AA64PFR0_EL1 and AA64ISAR1_EL1 here; they might even be available
        // at EL0 if HWCAP_CPUID is set in AT_HWCAP, but being no_std means
        // this code might be called in a context where we cannot call into
        // the libc to obtain the auxv (and if we could, we could read from
        // AT_HWCAP the HWCAP_DIT and HWCAP_SB bits directly).
        //
        // The best that can be done, without adding several ARM-specific
        // features to specify "this code will run at EL1" or "this code
        // will run under a Linux kernel greater than 4.11", is to use what's
        // known to be implemented at compile time, and allow an override
        // through the undocumented `set_aarch64_dit_sb_features` function.

        // SAFETY: each parameter is true only if the feature is implemented
        unsafe {
            set_aarch64_dit_sb_features(cfg!(target_feature = "dit"), cfg!(target_feature = "sb"))
        }
    }
}

/// Overrides the runtime detection of `FEAT_DIT` and `FEAT_SB`.
///
/// This must be called before other threads are created, and before
/// any other code in this `constant_time_eq` crate is called.
///
/// # Safety
///
/// Either parameter must not be set to true if the corresponding feature
/// is not implemented.
pub unsafe fn set_aarch64_dit_sb_features(_dit: bool, _sb: bool) {
    // SAFETY: the safety requirements are the same as this function
    #[cfg(not(all(target_feature = "dit", target_feature = "sb")))]
    unsafe {
        detect::set_aarch64_dit_sb_features(_dit, _sb);
    }
}

#[cfg(not(all(target_feature = "dit", target_feature = "sb")))]
use detect::get_aarch64_dit_sb_features;

/// Determines whether `FEAT_DIT` and `FEAT_SB` are known to be implemented.
#[cfg(all(target_feature = "dit", target_feature = "sb"))]
#[inline(always)]
fn get_aarch64_dit_sb_features() -> Features {
    // Both are known to be implemented at compile time, skip the detection.
    Features::DitSb
}

/// Equivalent to `__arm_rsr64("dit")`.
///
/// # Safety
///
/// Must be called only when `FEAT_DIT` is implemented.
#[inline]
#[target_feature(enable = "dit")]
unsafe fn rsr64_dit() -> u64 {
    let mut value;
    // SAFETY: called only when `FEAT_DIT` is implemented
    unsafe {
        asm!("mrs {}, dit", lateout(reg) value, options(nomem, preserves_flags, nostack));
    }
    value
}

/// Equivalent to `__arm_wsr64("dit", value)`.
///
/// # Safety
///
/// Must be called only when `FEAT_DIT` is implemented.
#[inline]
#[target_feature(enable = "dit")]
unsafe fn wsr64_dit(value: u64) {
    // SAFETY: called only when `FEAT_DIT` is implemented
    unsafe {
        // The compiler must not cache values or flags across this instruction.
        asm!("msr dit, {}", in(reg) value, options(nostack));
    }
}

/// Equivalent to `__arm_wsr64("dit", 1 << 24)`.
///
/// # Safety
///
/// Must be called only when `FEAT_DIT` is implemented.
#[inline]
#[target_feature(enable = "dit")]
unsafe fn enable_dit() {
    // SAFETY: called only when `FEAT_DIT` is implemented
    unsafe {
        // The compiler must not cache values or flags across this instruction.
        asm!("msr dit, #{}", const 1, options(nostack));
    }
}

/// Equivalent to `__asm__ __volatile__("sb" ::: "memory")`.
///
/// # Safety
///
/// Must be called only when `FEAT_SB` is implemented.
#[inline]
#[target_feature(enable = "sb")]
unsafe fn speculation_barrier() {
    // SAFETY: called only when `FEAT_SB` is implemented
    unsafe {
        // The compiler must not cache values or flags across this instruction.
        asm!("sb", options(nostack));
    }
}

/// Synchronization barrier for when `FEAT_SB` is not available.
///
/// This heavier alternative is recommended by Apple when SB is not available, see:
/// <https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms>
#[inline]
fn synchronization_barrier() {
    // SAFETY: these instructions are always available, and have no effects
    // other than being a data barrier and flushing the pipeline
    unsafe {
        // The compiler must not cache values or flags across this instruction.
        asm!("dsb nsh", "isb sy", options(nostack));
    }
}

/// Wraps code with DIT when `FEAT_DIT` and `FEAT_SB` were detected.
///
/// # Safety
///
/// `FEAT_DIT` and `FEAT_SB` must have been detected.
#[inline]
#[target_feature(enable = "dit,sb")]
unsafe fn with_feat_dit_sb<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    struct Guard {
        dit: u64,
    }

    impl Drop for Guard {
        #[inline]
        fn drop(&mut self) {
            // SAFETY: called only when `FEAT_DIT` is implemented
            unsafe { wsr64_dit(self.dit) };
        }
    }

    let _guard = Guard {
        // SAFETY: called only when `FEAT_DIT` is implemented
        dit: unsafe { rsr64_dit() },
    };

    // SAFETY: called only when `FEAT_DIT` is implemented
    unsafe { enable_dit() };

    // SAFETY: called only when `FEAT_SB` is implemented
    unsafe { speculation_barrier() };

    f()
}

/// Wraps code with DIT when `FEAT_DIT` was detected but not `FEAT_SB`.
///
/// # Safety
///
/// `FEAT_DIT` must have been detected.
#[inline]
#[target_feature(enable = "dit")]
unsafe fn with_feat_dit<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    struct Guard {
        dit: u64,
    }

    impl Drop for Guard {
        #[inline]
        fn drop(&mut self) {
            // SAFETY: called only when `FEAT_DIT` is implemented
            unsafe { wsr64_dit(self.dit) };
        }
    }

    let _guard = Guard {
        // SAFETY: called only when `FEAT_DIT` is implemented
        dit: unsafe { rsr64_dit() },
    };

    // SAFETY: called only when `FEAT_DIT` is implemented
    unsafe { enable_dit() };

    synchronization_barrier();

    f()
}

/// Runs code with the hardware DIT feature enabled when possible.
#[inline]
pub(crate) fn with_dit<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    // The use of #[target_feature] disables inlining in some cases.
    // Repeating the code three times with different #[target_feature]
    // generates better code.
    match get_aarch64_dit_sb_features() {
        Features::DitSb => {
            // SAFETY: both `FEAT_DIT` and `FEAT_SB` were detected
            unsafe { with_feat_dit_sb(f) }
        }
        Features::DitOnly => {
            // SAFETY: `FEAT_DIT` was detected
            unsafe { with_feat_dit(f) }
        }
        Features::Neither => f(),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::{rsr64_dit, with_dit};
    use std::arch::is_aarch64_feature_detected;

    #[test]
    fn dit_is_restored_after_with_dit() {
        if is_aarch64_feature_detected!("dit") {
            // SAFETY: `FEAT_DIT` was detected
            unsafe {
                let saved = rsr64_dit();
                with_dit(|| assert_ne!(rsr64_dit(), 0));
                assert_eq!(rsr64_dit(), saved);
            }
        }
    }
}
