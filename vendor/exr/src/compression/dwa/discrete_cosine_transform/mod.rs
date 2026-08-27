// Discrete cosine transform (forward and inverse) for DWA, ported from
// OpenEXRCore's internal_dwa_simd.h, including its runtime CPU dispatch:
// `dct_inverse_8x8_batch`/`dct_forward_8x8_batch` pick the best available x86
// tier at runtime (avx2 > sse2 > scalar), like OpenEXRs cpuid-based
// `initializeFuncs`
//
// Dispatch uses pulp's V3/V1 tokens, constructed only after a runtime CPU
// feature check. V3 needs AVX2+FMA, so
// AVX-only CPUs (Sandy/Ivy Bridge) fall back to sse2 here, where OpenEXR
// would use its own (non-avx2) avx kernel.
//
// The three kernels aren't bit-identical to each other (OpenEXRs own
// kernels disagree too: basis-constant precision and summation order
// differ)

// public only for benchmarking (the runtime dispatch below and the tier tests
// reach the individual kernels through this)
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[doc(hidden)]
pub mod x86;

// public only for benchmarking (benches/dct.rs reaches
// `test::pseudo_random_blocks`)
#[cfg(any(test, feature = "simd-benches"))]
#[doc(hidden)]
pub mod test;

// Autovectorized fallback: OpenEXRs "dctInverse8x8_scalar", including its
// truncated PI constant and summation order. Written as straightforward
// fixed-size loops that LLVM can autovectorize without explicit SIMD.
// public only for benchmarking (the in-crate dispatch and tests reach it
// directly)
#[doc(hidden)]
pub fn dct_inverse_8x8_autovectorized(data: &mut [f32; 64]) {
    const PI: f32 = 3.14159;

    let a = 0.5 * (PI / 4.0).cos();
    let b = 0.5 * (PI / 16.0).cos();
    let c = 0.5 * (PI / 8.0).cos();
    let d = 0.5 * ((3.0 * PI) / 16.0).cos();
    let e = 0.5 * ((5.0 * PI) / 16.0).cos();
    let f = 0.5 * ((3.0 * PI) / 8.0).cos();
    let g = 0.5 * ((7.0 * PI) / 16.0).cos();

    let mut alpha = [0f32; 4];
    let mut beta = [0f32; 4];
    let mut theta = [0f32; 4];
    let mut gamma = [0f32; 4];

    // First pass; row wise
    for row in 0..8 {
        let base = row * 8;
        let row_ptr = &mut data[base..base + 8];

        alpha[0] = c * row_ptr[2];
        alpha[1] = f * row_ptr[2];
        alpha[2] = c * row_ptr[6];
        alpha[3] = f * row_ptr[6];

        beta[0] = b * row_ptr[1] + d * row_ptr[3] + e * row_ptr[5] + g * row_ptr[7];
        beta[1] = d * row_ptr[1] - g * row_ptr[3] - b * row_ptr[5] - e * row_ptr[7];
        beta[2] = e * row_ptr[1] - b * row_ptr[3] + g * row_ptr[5] + d * row_ptr[7];
        beta[3] = g * row_ptr[1] - e * row_ptr[3] + d * row_ptr[5] - b * row_ptr[7];

        theta[0] = a * (row_ptr[0] + row_ptr[4]);
        theta[3] = a * (row_ptr[0] - row_ptr[4]);

        theta[1] = alpha[0] + alpha[3];
        theta[2] = alpha[1] - alpha[2];

        gamma[0] = theta[0] + theta[1];
        gamma[1] = theta[3] + theta[2];
        gamma[2] = theta[3] - theta[2];
        gamma[3] = theta[0] - theta[1];

        row_ptr[0] = gamma[0] + beta[0];
        row_ptr[1] = gamma[1] + beta[1];
        row_ptr[2] = gamma[2] + beta[2];
        row_ptr[3] = gamma[3] + beta[3];

        row_ptr[4] = gamma[3] - beta[3];
        row_ptr[5] = gamma[2] - beta[2];
        row_ptr[6] = gamma[1] - beta[1];
        row_ptr[7] = gamma[0] - beta[0];
    }

    // Second pass; column wise
    for column in 0..8 {
        alpha[0] = c * data[16 + column];
        alpha[1] = f * data[16 + column];
        alpha[2] = c * data[48 + column];
        alpha[3] = f * data[48 + column];

        beta[0] = b * data[8 + column]
            + d * data[24 + column]
            + e * data[40 + column]
            + g * data[56 + column];

        beta[1] = d * data[8 + column]
            - g * data[24 + column]
            - b * data[40 + column]
            - e * data[56 + column];

        beta[2] = e * data[8 + column] - b * data[24 + column]
            + g * data[40 + column]
            + d * data[56 + column];

        beta[3] = g * data[8 + column] - e * data[24 + column] + d * data[40 + column]
            - b * data[56 + column];

        theta[0] = a * (data[column] + data[32 + column]);
        theta[3] = a * (data[column] - data[32 + column]);

        theta[1] = alpha[0] + alpha[3];
        theta[2] = alpha[1] - alpha[2];

        gamma[0] = theta[0] + theta[1];
        gamma[1] = theta[3] + theta[2];
        gamma[2] = theta[3] - theta[2];
        gamma[3] = theta[0] - theta[1];

        data[column] = gamma[0] + beta[0];
        data[8 + column] = gamma[1] + beta[1];
        data[16 + column] = gamma[2] + beta[2];
        data[24 + column] = gamma[3] + beta[3];

        data[32 + column] = gamma[3] - beta[3];
        data[40 + column] = gamma[2] - beta[2];
        data[48 + column] = gamma[1] - beta[1];
        data[56 + column] = gamma[0] - beta[0];
    }
}

/// Autovectorized forward DCT for DWA 8x8 blocks. This intentionally uses the
/// straightforward separable DCT formula for the first encoder version; LLVM
/// can still optimize the fixed-size loops without adding explicit SIMD paths.
// public only for benchmarking (the in-crate dispatch and tests reach it directly)
#[doc(hidden)]
pub fn dct_forward_8x8_autovectorized(data: &mut [f32; 64]) {
    // The forward path mirrors the inverse path's fixed 8x8 basis, but keeps
    // the implementation autovectorized and easy to verify against the reference.
    const PI: f32 = 3.14159;
    const INV_SQRT_2: f32 = 0.70710677;

    let input = *data;
    for v in 0..8 {
        for u in 0..8 {
            let cu = if u == 0 {
                INV_SQRT_2
            } else {
                1.0
            };
            let cv = if v == 0 {
                INV_SQRT_2
            } else {
                1.0
            };
            let mut sum = 0.0f32;

            for y in 0..8 {
                let cy = (((2 * y + 1) as f32 * v as f32 * PI) / 16.0).cos();
                for x in 0..8 {
                    let cx = (((2 * x + 1) as f32 * u as f32 * PI) / 16.0).cos();
                    sum += input[y * 8 + x] * cx * cy;
                }
            }

            data[v * 8 + u] = 0.25 * cu * cv * sum;
        }
    }
}

/// Forward DCT on many 8x8 blocks, dispatched once for the whole batch rather
/// than once per block. Prefer this over looping calls to `dct_forward_8x8`.
pub(crate) fn dct_forward_8x8_batch<'a>(mut blocks: impl Iterator<Item = &'a mut [f32; 64]>) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_dct_forward_8x8_batch(&mut blocks) {
        return;
    }

    for data in blocks {
        dct_forward_8x8_autovectorized(data);
    }
}

/// Inverse DCT on many 8x8 blocks, dispatched once for the whole batch
/// rather than once per block. Prefer this over looping calls to
/// `dct_inverse_8x8`
pub(crate) fn dct_inverse_8x8_batch<'a>(mut blocks: impl Iterator<Item = &'a mut [f32; 64]>) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if x86::try_dct_inverse_8x8_batch(&mut blocks) {
        return;
    }

    for data in blocks {
        dct_inverse_8x8_autovectorized(data);
    }
}

/// Optimized path when only DC is non-zero.
pub(crate) fn dct_inverse_8x8_dc_only(data: &mut [f32; 64]) {
    let val = data[0] * 0.3535536f32 * 0.3535536f32;
    for v in data.iter_mut() {
        *v = val;
    }
}
