//! SSE2/AVX implementation of `constant_time_eq` and `constant_time_eq_n`.
//!
//! Note: some microarchitectures split vector operations and/or vector registers larger than
//! 128-bit, and might have optimizations for when one of the halves is all-zeros. To protect
//! against that, only 128-bit vectors are used, even though larger vectors might be faster.

use core::arch::asm;
use core::mem::size_of;

#[cfg(target_arch = "x86")]
use core::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::with_dit;

/// Equivalent to `_mm_cmpeq_epi8`, but hidden from the compiler.
///
/// The use of inline assembly instead of an intrinsic prevents a sufficiently
/// smart compiler from computing the mask in other ways which might not be
/// constant time (for instance, looping through the input and using branching
/// to set the vector elements).
#[must_use]
#[inline(always)]
fn cmpeq_epi8(a: __m128i, b: __m128i) -> __m128i {
    let mut c;
    // When AVX is available, the compiler will use the VEX prefix for all
    // SIMD instructions; do the same for this inline assembly.
    if cfg!(target_feature = "avx") {
        // SAFETY: used only when AVX is available
        // SAFETY: assembly instruction touches only these registers
        unsafe {
            asm!("vpcmpeqb {c}, {a}, {b}",
                c = lateout(xmm_reg) c,
                a = in(xmm_reg) a,
                b = in(xmm_reg) b,
                options(pure, nomem, preserves_flags, nostack));
        }
    } else {
        // SAFETY: this file is compiled only when SSE2 is available
        // SAFETY: assembly instruction touches only these registers
        unsafe {
            asm!("pcmpeqb {a}, {b}",
                a = inlateout(xmm_reg) a => c,
                b = in(xmm_reg) b,
                options(pure, nomem, preserves_flags, nostack));
        }
    }
    c
}

/// Equivalent to `_mm_and_si128`, but hidden from the compiler.
///
/// The use of inline assembly instead of an intrinsic prevents a sufficiently
/// smart compiler from short circuiting the computation once the mask becomes
/// all zeros.
#[must_use]
#[inline(always)]
fn and_si128(a: __m128i, b: __m128i) -> __m128i {
    let mut c;
    // When AVX is available, the compiler will use the VEX prefix for all
    // SIMD instructions; do the same for this inline assembly.
    if cfg!(target_feature = "avx") {
        // SAFETY: used only when AVX is available
        // SAFETY: assembly instruction touches only these registers
        unsafe {
            asm!("vpand {c}, {a}, {b}",
                c = lateout(xmm_reg) c,
                a = in(xmm_reg) a,
                b = in(xmm_reg) b,
                options(pure, nomem, preserves_flags, nostack));
        }
    } else {
        // SAFETY: this file is compiled only when SSE2 is available
        // SAFETY: assembly instruction touches only these registers
        unsafe {
            asm!("pand {a}, {b}",
                a = inlateout(xmm_reg) a => c,
                b = in(xmm_reg) b,
                options(pure, nomem, preserves_flags, nostack));
        }
    }
    c
}

/// Equivalent to `_mm_movemask_epi8`, but hidden from the compiler.
///
/// The use of inline assembly instead of an intrinsic prevents a sufficiently
/// smart compiler from extracting the mask in other ways which might not be
/// constant time (for instance, looping through the elements of the vector).
#[must_use]
#[inline(always)]
fn movemask_epi8(a: __m128i) -> u32 {
    let mut mask;
    // When AVX is available, the compiler will use the VEX prefix for all
    // SIMD instructions; do the same for this inline assembly.
    if cfg!(target_feature = "avx") {
        // SAFETY: used only when AVX is available
        // SAFETY: assembly instruction touches only these registers
        // SAFETY: 32-bit operations zero-extend the 64-bit register
        unsafe {
            asm!("vpmovmskb {mask:e}, {a}",
                mask = lateout(reg) mask,
                a = in(xmm_reg) a,
                options(pure, nomem, preserves_flags, nostack));
        }
    } else {
        // SAFETY: this file is compiled only when SSE2 is available
        // SAFETY: assembly instruction touches only these registers
        // SAFETY: 32-bit operations zero-extend the 64-bit register
        unsafe {
            asm!("pmovmskb {mask:e}, {a}",
                mask = lateout(reg) mask,
                a = in(xmm_reg) a,
                options(pure, nomem, preserves_flags, nostack));
        }
    }
    // The return type is u32 instead of i32 to avoid a sign extension.
    mask
}

/// Safe equivalent to `_mm_loadu_si128` for byte slices.
#[must_use]
#[inline(always)]
fn loadu_si128(src: &[u8]) -> __m128i {
    assert_eq!(src.len(), size_of::<__m128i>());

    // SAFETY: this file is compiled only when SSE2 is available
    // SAFETY: the slice has enough bytes for a __m128i
    unsafe { _mm_loadu_si128(src.as_ptr().cast::<__m128i>()) }
}

/// SSE2/AVX implementation of `constant_time_eq` and `constant_time_eq_n`.
#[must_use]
#[inline(always)]
fn constant_time_eq_sse2(mut a: &[u8], mut b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // This statement does nothing, because a.len() == b.len() here,
    // but it makes the optimizer elide some useless bounds checks.
    b = &b[..a.len()];

    const LANES: usize = size_of::<__m128i>();

    let tmp = if a.len() >= LANES * 2 {
        let tmpa0 = loadu_si128(&a[..LANES]);
        let tmpb0 = loadu_si128(&b[..LANES]);
        let tmpa1 = loadu_si128(&a[LANES..LANES * 2]);
        let tmpb1 = loadu_si128(&b[LANES..LANES * 2]);

        a = &a[LANES * 2..];
        b = &b[LANES * 2..];

        let mut mask0 = cmpeq_epi8(tmpa0, tmpb0);
        let mut mask1 = cmpeq_epi8(tmpa1, tmpb1);

        while a.len() >= LANES * 2 {
            let tmpa0 = loadu_si128(&a[..LANES]);
            let tmpb0 = loadu_si128(&b[..LANES]);
            let tmpa1 = loadu_si128(&a[LANES..LANES * 2]);
            let tmpb1 = loadu_si128(&b[LANES..LANES * 2]);

            a = &a[LANES * 2..];
            b = &b[LANES * 2..];

            let tmp0 = cmpeq_epi8(tmpa0, tmpb0);
            let tmp1 = cmpeq_epi8(tmpa1, tmpb1);

            mask0 = and_si128(mask0, tmp0);
            mask1 = and_si128(mask1, tmp1);
        }

        if a.len() >= LANES {
            let tmpa = loadu_si128(&a[..LANES]);
            let tmpb = loadu_si128(&b[..LANES]);

            a = &a[LANES..];
            b = &b[LANES..];

            let tmp = cmpeq_epi8(tmpa, tmpb);

            mask0 = and_si128(mask0, tmp);
        }

        let mask = and_si128(mask0, mask1);
        movemask_epi8(mask) ^ 0xFFFF
    } else if a.len() >= LANES {
        let tmpa = loadu_si128(&a[..LANES]);
        let tmpb = loadu_si128(&b[..LANES]);

        a = &a[LANES..];
        b = &b[LANES..];

        let mask = cmpeq_epi8(tmpa, tmpb);

        movemask_epi8(mask) ^ 0xFFFF
    } else {
        0
    };

    // Note: be careful to not short-circuit ("tmp == 0 &&") the comparison here
    crate::generic::constant_time_eq_impl(a, b, tmp.into())
}

#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    with_dit(|| constant_time_eq_sse2(a, b))
}

#[must_use]
pub fn constant_time_eq_n<const N: usize>(a: &[u8; N], b: &[u8; N]) -> bool {
    with_dit(|| constant_time_eq_sse2(&a[..], &b[..]))
}
