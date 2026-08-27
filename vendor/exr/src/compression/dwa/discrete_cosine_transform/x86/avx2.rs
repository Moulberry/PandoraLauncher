// AVX2 V3 tier: OpenEXR's "dctInverse8x8_avx_0". Each pass runs all 8
// rows/columns of the block in parallel, one 8-wide register per position.
//
// `dct_inverse_8x8_batch` runs the kernel through `V3::vectorize` rather
// than calling it as an ordinary function. `vectorize` is pulps own
// inherent, `#[target_feature]`-scoped, internally-unsafe trampoline

use pulp::{f32x8, x86::V3};

use super::forward_basis;

// OpenEXRs hardcoded AVX basis constants ("sAvxCoef").
const A: f32 = 3.535536e-1;
const B: f32 = 4.903927e-1;
const C: f32 = 4.619398e-1;
const D: f32 = 4.157349e-1;
const E: f32 = 2.777855e-1;
const F: f32 = 1.913422e-1;
const G: f32 = 9.754573e-2;

struct Coefficients {
    a: f32x8,
    na: f32x8,
    b: f32x8,
    nb: f32x8,
    c: f32x8,
    nc: f32x8,
    d: f32x8,
    // no "nd": the AVX never multiplies by -D
    e: f32x8,
    ne: f32x8,
    f: f32x8,
    nf: f32x8,
    g: f32x8,
    ng: f32x8,
}

impl Coefficients {
    // This, `row_pass`, and `column_pass` must inline into the
    // `vectorize` closure below for their ops to fuse into avx2
    // instructions; LLVM inlining heuristics aren't reliable
    // enough to guarantee that on their own
    #[inline(always)]
    fn new(v3: V3) -> Self {
        // Negated splats are exact (sign flip), so "x * na == -(x * a)".
        Self {
            a: v3.splat_f32x8(A),
            na: v3.splat_f32x8(-A),
            b: v3.splat_f32x8(B),
            nb: v3.splat_f32x8(-B),
            c: v3.splat_f32x8(C),
            nc: v3.splat_f32x8(-C),
            d: v3.splat_f32x8(D),
            e: v3.splat_f32x8(E),
            ne: v3.splat_f32x8(-E),
            f: v3.splat_f32x8(F),
            nf: v3.splat_f32x8(-F),
            g: v3.splat_f32x8(G),
            ng: v3.splat_f32x8(-G),
        }
    }
}

// OpenEXRs "IDCT_AVX_MMULT_ROWS" + "EO_TO_ROW_HALVES"
#[inline(always)] // must fuse into the `vectorize` closure --> see `Coefficients::new`
fn row_pass(v3: V3, coef: &Coefficients, input: [f32x8; 8]) -> [f32x8; 8] {
    let mul = |a, b| v3.mul_f32x8(a, b);
    let add = |a, b| v3.add_f32x8(a, b);
    let sub = |a, b| v3.sub_f32x8(a, b);

    let (in0, in2, in4, in6) = (input[0], input[2], input[4], input[6]);
    let (in1, in3, in5, in7) = (input[1], input[3], input[5], input[7]);

    let even0 =
        add(add(mul(in4, coef.a), mul(in6, coef.f)), add(mul(in0, coef.a), mul(in2, coef.c)));
    let even1 =
        add(add(mul(in4, coef.na), mul(in6, coef.nc)), add(mul(in0, coef.a), mul(in2, coef.f)));
    let even2 =
        add(add(mul(in4, coef.na), mul(in6, coef.c)), add(mul(in0, coef.a), mul(in2, coef.nf)));
    let even3 =
        add(add(mul(in4, coef.a), mul(in6, coef.nf)), add(mul(in0, coef.a), mul(in2, coef.nc)));

    let odd0 =
        add(add(mul(in5, coef.e), mul(in7, coef.g)), add(mul(in1, coef.b), mul(in3, coef.d)));
    let odd1 =
        add(add(mul(in5, coef.nb), mul(in7, coef.ne)), add(mul(in1, coef.d), mul(in3, coef.ng)));
    let odd2 =
        add(add(mul(in5, coef.g), mul(in7, coef.d)), add(mul(in1, coef.e), mul(in3, coef.nb)));
    let odd3 =
        add(add(mul(in5, coef.d), mul(in7, coef.nb)), add(mul(in1, coef.g), mul(in3, coef.ne)));

    [
        add(even0, odd0),
        add(even1, odd1),
        add(even2, odd2),
        add(even3, odd3),
        sub(even3, odd3),
        sub(even2, odd2),
        sub(even1, odd1),
        sub(even0, odd0),
    ]
}

// The column transform from the back half of "dctInverse8x8_avx_0".
#[inline(always)] // must fuse into the `vectorize` closure --> see `Coefficients::new`
fn column_pass(v3: V3, coef: &Coefficients, input: [f32x8; 8]) -> [f32x8; 8] {
    let mul = |a, b| v3.mul_f32x8(a, b);
    let add = |a, b| v3.add_f32x8(a, b);
    let sub = |a, b| v3.sub_f32x8(a, b);

    let (in0, in1, in2, in3, in4, in5, in6, in7) =
        (input[0], input[1], input[2], input[3], input[4], input[5], input[6], input[7]);

    let beta0 =
        add(add(mul(coef.g, in7), mul(coef.e, in5)), add(mul(coef.d, in3), mul(coef.b, in1)));
    let beta1 =
        sub(sub(mul(coef.d, in1), add(mul(coef.b, in5), mul(coef.g, in3))), mul(coef.e, in7));
    let beta2 =
        add(mul(coef.d, in7), add(mul(coef.g, in5), sub(mul(coef.e, in1), mul(coef.b, in3))));
    let beta3 =
        sub(add(mul(coef.d, in5), mul(coef.g, in1)), add(mul(coef.b, in7), mul(coef.e, in3)));

    let theta0 = add(mul(coef.a, in4), mul(coef.a, in0));
    let theta3 = sub(mul(coef.a, in0), mul(coef.a, in4));

    let theta1 = add(mul(coef.f, in6), mul(coef.c, in2));
    let gamma0 = add(theta1, theta0);
    let gamma3 = sub(theta0, theta1);

    let theta2 = sub(mul(coef.f, in2), mul(coef.c, in6));
    let gamma1 = add(theta3, theta2);
    let gamma2 = sub(theta3, theta2);

    [
        add(gamma0, beta0),
        add(gamma1, beta1),
        add(gamma2, beta2),
        add(gamma3, beta3),
        sub(gamma3, beta3),
        sub(gamma2, beta2),
        sub(gamma1, beta1),
        sub(gamma0, beta0),
    ]
}

#[cfg(any(feature = "avx2-tests", feature = "simd-benches"))]
pub fn dct_inverse_8x8(v3: V3, data: &mut [f32; 64]) {
    dct_inverse_8x8_batch(v3, std::iter::once(data));
}

// `V3::vectorize` runs a `FnOnce()` closure inside pulps own
// `#[target_feature(enable = "avx2,fma")]` trampoline; passing the
// kernel as a closure, rather than calling it as an ordinary function,
// is what lets that closures body inline and fuse into avx2
// instructions.
//
// `vectorize` also has fixed overhead per call (an indirect call
// through its register-passing trampoline, plus a feature-cache check)
pub fn dct_inverse_8x8_batch<'a>(v3: V3, blocks: impl Iterator<Item = &'a mut [f32; 64]>) {
    v3.vectorize(move || {
        let coef = Coefficients::new(v3);

        for data in blocks {
            // Row pass: lane i = row i, gathered with a strided read
            // (data is row-major).
            let columns: [f32x8; 8] = std::array::from_fn(|k| {
                f32x8(
                    data[k],
                    data[8 + k],
                    data[16 + k],
                    data[24 + k],
                    data[32 + k],
                    data[40 + k],
                    data[48 + k],
                    data[56 + k],
                )
            });

            let rows_out = row_pass(v3, &coef, columns);
            for (column, result) in rows_out.iter().enumerate() {
                let r = [
                    result.0, result.1, result.2, result.3, result.4, result.5, result.6, result.7,
                ];
                for (row, value) in r.iter().enumerate() {
                    data[row * 8 + column] = *value;
                }
            }

            // Column pass: lane i = column i, each row already contiguous.
            let rows: [f32x8; 8] = std::array::from_fn(|row| {
                let b = row * 8;
                f32x8(
                    data[b],
                    data[b + 1],
                    data[b + 2],
                    data[b + 3],
                    data[b + 4],
                    data[b + 5],
                    data[b + 6],
                    data[b + 7],
                )
            });

            let columns_out = column_pass(v3, &coef, rows);
            for (row, result) in columns_out.iter().enumerate() {
                let b = row * 8;
                data[b] = result.0;
                data[b + 1] = result.1;
                data[b + 2] = result.2;
                data[b + 3] = result.3;
                data[b + 4] = result.4;
                data[b + 5] = result.5;
                data[b + 6] = result.6;
                data[b + 7] = result.7;
            }
        }
    });
}

struct ForwardCoefficients {
    terms: [f32x8; 8],
}

impl ForwardCoefficients {
    #[inline(always)]
    fn new(_v3: V3) -> Self {
        let basis = forward_basis();
        Self {
            terms: std::array::from_fn(|input| {
                let row = basis[input];
                f32x8(row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7])
            }),
        }
    }
}

#[inline(always)]
fn forward_pass(v3: V3, coef: &ForwardCoefficients, input: [f32; 8]) -> f32x8 {
    let mul = |a, b| v3.mul_f32x8(a, b);
    let add = |a, b| v3.add_f32x8(a, b);
    let splat = |value: f32| v3.splat_f32x8(value);

    let mut out = v3.splat_f32x8(0.0);
    for index in 0..8 {
        out = add(out, mul(splat(input[index]), coef.terms[index]));
    }
    out
}

// TODO just #[test]
#[cfg(any(feature = "avx2-tests", feature = "simd-benches"))]
pub fn dct_forward_8x8(v3: V3, data: &mut [f32; 64]) {
    dct_forward_8x8_batch(v3, std::iter::once(data));
}

pub fn dct_forward_8x8_batch<'a>(v3: V3, blocks: impl Iterator<Item = &'a mut [f32; 64]>) {
    v3.vectorize(move || {
        let coef = ForwardCoefficients::new(v3);

        for data in blocks {
            for row in 0..8 {
                let base = row * 8;
                let input = [
                    data[base],
                    data[base + 1],
                    data[base + 2],
                    data[base + 3],
                    data[base + 4],
                    data[base + 5],
                    data[base + 6],
                    data[base + 7],
                ];
                let out = forward_pass(v3, &coef, input);
                data[base] = out.0;
                data[base + 1] = out.1;
                data[base + 2] = out.2;
                data[base + 3] = out.3;
                data[base + 4] = out.4;
                data[base + 5] = out.5;
                data[base + 6] = out.6;
                data[base + 7] = out.7;
            }

            for column in 0..8 {
                let input = [
                    data[column],
                    data[8 + column],
                    data[16 + column],
                    data[24 + column],
                    data[32 + column],
                    data[40 + column],
                    data[48 + column],
                    data[56 + column],
                ];
                let out = forward_pass(v3, &coef, input);
                data[column] = out.0;
                data[8 + column] = out.1;
                data[16 + column] = out.2;
                data[24 + column] = out.3;
                data[32 + column] = out.4;
                data[40 + column] = out.5;
                data[48 + column] = out.6;
                data[56 + column] = out.7;
            }
        }
    });
}
