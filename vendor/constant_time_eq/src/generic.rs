//! Generic implementation of `constant_time_eq` and `constant_time_eq_n`.
//!
//! This implementation does SIMD in general-purpose registers instead of vector registers, and
//! uses inline assembly only to hide the dependencies and comparisons from the optimizer, to
//! prevent it from returning early when the accumulator becomes non-zero (found a difference) or
//! all-ones (the accumulator can no longer change).
//!
//! This generic implementation is also used for suffixes smaller than one vector from the
//! architecture-specific vector implementations. This is simpler and often faster than trying to
//! load a partial vector register.

use core::mem::size_of;
use core::ops::BitXor;
use core::ptr::read_unaligned;

use crate::with_dit;

/// The natural word type for this architecture. All bit patterns must be valid for this type.
#[cfg(all(
    target_pointer_width = "64",
    any(not(target_arch = "riscv64"), target_feature = "unaligned-scalar-mem")
))]
pub(crate) type Word = u64;

/// The natural word type for this architecture. All bit patterns must be valid for this type.
#[cfg(all(
    target_pointer_width = "32",
    any(not(target_arch = "riscv32"), target_feature = "unaligned-scalar-mem")
))]
pub(crate) type Word = u32;

/// The natural word type for this architecture. All bit patterns must be valid for this type.
#[cfg(target_pointer_width = "16")]
pub(crate) type Word = u16;

/// The natural word type for this architecture. All bit patterns must be valid for this type.
#[cfg(not(any(
    target_pointer_width = "64",
    target_pointer_width = "32",
    target_pointer_width = "16"
)))]
pub(crate) type Word = usize;

// RISC-V without unaligned-scalar-mem generates worse code for unaligned word reads.
/// The natural word type for this architecture. All bit patterns must be valid for this type.
#[cfg(all(
    any(target_arch = "riscv64", target_arch = "riscv32"),
    not(target_feature = "unaligned-scalar-mem")
))]
pub(crate) type Word = u8;

/// Hides a value from the optimizer.
#[cfg(all(
    not(miri),
    any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
    )
))]
#[must_use]
#[inline(always)]
fn optimizer_hide(mut value: Word) -> Word {
    // SAFETY: the input value is passed unchanged to the output, the inline assembly does nothing.
    unsafe {
        core::arch::asm!("/* {0} */", inlateout(reg) value, options(pure, nomem, preserves_flags, nostack));
    }
    value
}

/// Attempts to hide a value from the optimizer.
#[cfg(any(
    miri,
    not(any(
        target_arch = "x86",
        target_arch = "x86_64",
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "riscv32",
        target_arch = "riscv64",
        target_arch = "loongarch64",
        target_arch = "s390x",
    ))
))]
#[must_use]
#[inline(never)]
fn optimizer_hide(value: Word) -> Word {
    // The current implementation of black_box in the main codegen backends is similar to
    // {
    //     let result = value;
    //     asm!("", in(reg) &result);
    //     result
    // }
    // which round-trips the value through the stack, instead of leaving it in a register.
    // Experimental codegen backends might implement black_box as a pure identity function,
    // without the expected optimization barrier, so it's less guaranteed than inline asm.
    // For that reason, we also use the #[inline(never)] hint, which makes it harder for an
    // optimizer to look inside this function.
    core::hint::black_box(value)
}

/// Equivalent to `read_unaligned` for byte slices.
///
/// # Safety
///
/// All bit patterns must be valid for type T.
#[must_use]
#[inline(always)]
unsafe fn read_unaligned_from_slice<T>(src: &[u8]) -> T {
    assert_eq!(src.len(), size_of::<T>());

    // SAFETY: the slice has enough bytes for type T
    // SAFETY: all bit patterns are valid for type T
    unsafe { read_unaligned(src.as_ptr().cast::<T>()) }
}

/// Generic implementation of `constant_time_eq` and `constant_time_eq_n`.
#[must_use]
#[inline(always)]
pub(crate) fn constant_time_eq_impl(mut a: &[u8], mut b: &[u8], mut tmp: Word) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // This statement does nothing, because a.len() == b.len() here,
    // but it makes the optimizer elide some useless bounds checks.
    b = &b[..a.len()];

    // Early exit for the common case when called by the SIMD code.
    if a.is_empty() {
        return tmp == 0;
    }

    /// Reads and compares a single word from the input, adjusting the slices.
    /// Returns zero if both words are equal, non-zero if any byte is different.
    ///
    /// # Safety
    ///
    /// All bit patterns must be valid for type T.
    #[must_use]
    #[inline(always)]
    unsafe fn cmp_step<T: BitXor<Output = T>>(a: &mut &[u8], b: &mut &[u8]) -> T {
        // SAFETY: all bit patterns are valid for type T
        let tmpa = unsafe { read_unaligned_from_slice::<T>(&a[..size_of::<T>()]) };
        // SAFETY: all bit patterns are valid for type T
        let tmpb = unsafe { read_unaligned_from_slice::<T>(&b[..size_of::<T>()]) };

        *a = &a[size_of::<T>()..];
        *b = &b[size_of::<T>()..];

        tmpa ^ tmpb
    }

    // The optimizer is not allowed to assume anything about the value of tmp after each iteration,
    // which prevents it from terminating the loop early if the value becomes non-zero or all-ones.

    // Do most of the work using the natural word size; the other blocks clean up the leftovers.
    while a.len() >= size_of::<Word>() {
        // SAFETY: all bit patterns are valid for Word
        let cmp = optimizer_hide(unsafe { cmp_step::<Word>(&mut a, &mut b) });
        tmp = optimizer_hide(tmp | cmp);
    }

    // These first two blocks would only be necessary for architectures with usize > 64 bits.
    // They are kept here for future-proofing, so that everything still works in that case.
    // The optimizer tracks the range of len and will not generate any code for these blocks.
    while a.len() >= size_of::<u128>() {
        // SAFETY: all bit patterns are valid for u128
        let cmp = optimizer_hide(unsafe { cmp_step::<u128>(&mut a, &mut b) } as Word);
        tmp = optimizer_hide(tmp | cmp);
    }
    if a.len() >= size_of::<u64>() {
        // SAFETY: all bit patterns are valid for u64
        let cmp = optimizer_hide(unsafe { cmp_step::<u64>(&mut a, &mut b) } as Word);
        tmp = optimizer_hide(tmp | cmp);
    }
    if a.len() >= size_of::<u32>() {
        // SAFETY: all bit patterns are valid for u32
        let cmp = optimizer_hide(unsafe { cmp_step::<u32>(&mut a, &mut b) } as Word);
        tmp = optimizer_hide(tmp | cmp);
    }
    if a.len() >= size_of::<u16>() {
        // SAFETY: all bit patterns are valid for u16
        let cmp = optimizer_hide(unsafe { cmp_step::<u16>(&mut a, &mut b) } as Word);
        tmp = optimizer_hide(tmp | cmp);
    }
    if a.len() >= size_of::<u8>() {
        // SAFETY: all bit patterns are valid for u8
        let cmp = optimizer_hide(unsafe { cmp_step::<u8>(&mut a, &mut b) } as Word);
        tmp = optimizer_hide(tmp | cmp);
    }

    tmp == 0
}

/// Compares two equal-sized byte strings in constant time.
///
/// # Examples
///
/// ```
/// use constant_time_eq::constant_time_eq;
///
/// assert!(constant_time_eq(b"foo", b"foo"));
/// assert!(!constant_time_eq(b"foo", b"bar"));
/// assert!(!constant_time_eq(b"bar", b"baz"));
/// # assert!(constant_time_eq(b"", b""));
///
/// // Not equal-sized, so won't take constant time.
/// assert!(!constant_time_eq(b"foo", b""));
/// assert!(!constant_time_eq(b"foo", b"quux"));
/// ```
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    with_dit(|| constant_time_eq_impl(a, b, 0))
}

/// Compares two fixed-size byte strings in constant time.
///
/// # Examples
///
/// ```
/// use constant_time_eq::constant_time_eq_n;
///
/// assert!(constant_time_eq_n(&[3; 20], &[3; 20]));
/// assert!(!constant_time_eq_n(&[3; 20], &[7; 20]));
/// ```
#[must_use]
pub fn constant_time_eq_n<const N: usize>(a: &[u8; N], b: &[u8; N]) -> bool {
    with_dit(|| constant_time_eq_impl(&a[..], &b[..], 0))
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "count_instructions_test")]
    extern crate std;

    #[cfg(feature = "count_instructions_test")]
    #[test]
    fn count_optimizer_hide_instructions() -> std::io::Result<()> {
        use super::{Word, optimizer_hide};
        use count_instructions::count_instructions;

        fn count() -> std::io::Result<usize> {
            // If optimizer_hide does not work, constant propagation and folding
            // will make this identical to count_optimized() below.
            let mut count = 0;
            assert_eq!(
                10 as Word,
                count_instructions(
                    || optimizer_hide(1)
                        + optimizer_hide(2)
                        + optimizer_hide(3)
                        + optimizer_hide(4),
                    |_| count += 1
                )?
            );
            Ok(count)
        }

        fn count_optimized() -> std::io::Result<usize> {
            #[inline(always)]
            fn inline_identity(value: Word) -> Word {
                value
            }

            let mut count = 0;
            assert_eq!(
                10 as Word,
                count_instructions(
                    || inline_identity(1)
                        + inline_identity(2)
                        + inline_identity(3)
                        + inline_identity(4),
                    |_| count += 1
                )?
            );
            Ok(count)
        }

        assert!(count()? > count_optimized()?);
        Ok(())
    }
}
