// Runtime x86 SIMD dispatch for the DWA DCT: `try_dct_*_8x8_batch` select the
// AVX2 tier when available and fall back to the SSE2 tier, otherwise let the
// caller use the scalar autovectorized path. Both tiers share the lazily-built
// `forward_basis` cosine table.

use std::sync::OnceLock;

use pulp::x86::{V1, V3};

// public only for benchmarking
#[doc(hidden)]
pub mod avx2;

// public only for benchmarking
#[doc(hidden)]
pub mod sse2;

pub(super) fn forward_basis() -> &'static [[f32; 8]; 8] {
    static TABLE: OnceLock<[[f32; 8]; 8]> = OnceLock::new();

    TABLE.get_or_init(|| {
        const PI: f32 = 3.14159;
        const INV_SQRT_2: f32 = 0.70710677;

        let mut table = [[0.0f32; 8]; 8];
        for input in 0..8 {
            for output in 0..8 {
                let scale = if output == 0 {
                    0.5 * INV_SQRT_2
                } else {
                    0.5
                };
                table[input][output] =
                    scale * (((2 * input + 1) as f32 * output as f32 * PI) / 16.0).cos();
            }
        }
        table
    })
}

pub(super) fn try_dct_forward_8x8_batch<'a, I>(blocks: &mut I) -> bool
where
    I: Iterator<Item = &'a mut [f32; 64]>,
{
    if let Some(v3) = V3::try_new() {
        avx2::dct_forward_8x8_batch(v3, blocks);
        return true;
    }
    if let Some(v1) = V1::try_new() {
        for data in blocks {
            sse2::dct_forward_8x8(v1, data);
        }
        return true;
    }
    false
}

pub(super) fn try_dct_inverse_8x8_batch<'a, I>(blocks: &mut I) -> bool
where
    I: Iterator<Item = &'a mut [f32; 64]>,
{
    if let Some(v3) = V3::try_new() {
        avx2::dct_inverse_8x8_batch(v3, blocks);
        return true;
    }
    if let Some(v1) = V1::try_new() {
        for data in blocks {
            sse2::dct_inverse_8x8(v1, data);
        }
        return true;
    }
    false
}
