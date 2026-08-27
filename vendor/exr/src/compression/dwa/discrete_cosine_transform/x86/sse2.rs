// SSE2 V1 tier: OpenEXRs "dctInverse8x8_sse2". Vectorizes 4
// output positions of one row at a time a different shape than "avx2",
// so the two are not bit-identical.

use pulp::{f32x4, x86::V1};

use super::forward_basis;

const A: f32 = 3.535536e-1;
const B: f32 = 4.903927e-1;
const C: f32 = 4.619398e-1;
const D: f32 = 4.157349e-1;
const E: f32 = 2.777855e-1;
const F: f32 = 1.913422e-1;
const G: f32 = 9.754573e-2;

// Row-pass matrix columns (c0..c3 even positions, c4..c7 odd).
struct RowCoefficients {
    c0: f32x4,
    c1: f32x4,
    c2: f32x4,
    c3: f32x4,
    c4: f32x4,
    c5: f32x4,
    c6: f32x4,
    c7: f32x4,
}

impl RowCoefficients {
    fn new() -> Self {
        Self {
            c0: f32x4(A, A, A, A),
            c1: f32x4(C, F, -F, -C),
            c2: f32x4(A, -A, -A, A),
            c3: f32x4(F, -C, C, -F),
            c4: f32x4(B, D, E, G),
            c5: f32x4(D, -G, -B, -E),
            c6: f32x4(E, -B, G, D),
            c7: f32x4(G, -E, D, -B),
        }
    }
}

struct ColumnCoefficients {
    a: f32x4,
    b: f32x4,
    c: f32x4,
    d: f32x4,
    e: f32x4,
    f: f32x4,
    g: f32x4,
}

impl ColumnCoefficients {
    fn new(v1: V1) -> Self {
        Self {
            a: v1.splat_f32x4(A),
            b: v1.splat_f32x4(B),
            c: v1.splat_f32x4(C),
            d: v1.splat_f32x4(D),
            e: v1.splat_f32x4(E),
            f: v1.splat_f32x4(F),
            g: v1.splat_f32x4(G),
        }
    }
}

// One row, summed strictly left-to-right from an explicit zero register,
// matching "DCT_INVERSE_8x8_SS2_ROW_LOOP"s "_mm_add_ps" exactly.
fn row_pass(v1: V1, coef: &RowCoefficients, row: &[f32]) -> (f32x4, f32x4) {
    let mul = |a, b| v1.mul_f32x4(a, b);
    let add = |a, b| v1.add_f32x4(a, b);
    let sub = |a, b| v1.sub_f32x4(a, b);
    let broadcast = |v: f32| v1.splat_f32x4(v);

    let x0 = mul(broadcast(row[0]), coef.c0);
    let x2 = mul(broadcast(row[2]), coef.c1);
    let x4 = mul(broadcast(row[4]), coef.c2);
    let x6 = mul(broadcast(row[6]), coef.c3);

    let x1 = mul(broadcast(row[1]), coef.c4);
    let x3 = mul(broadcast(row[3]), coef.c5);
    let x5 = mul(broadcast(row[5]), coef.c6);
    let x7 = mul(broadcast(row[7]), coef.c7);

    let zero = v1.splat_f32x4(0.0);
    let even = add(add(add(add(zero, x0), x2), x4), x6);
    let odd = add(add(add(add(zero, x1), x3), x5), x7);

    let lo = add(even, odd);
    let hi = sub(even, odd);
    (lo, f32x4(hi.3, hi.2, hi.1, hi.0))
}

// Same alpha/theta/gamma structure as the scalar kernel, but with
// beta0..beta3 tree-paired, matching "dctInverse8x8_sse2" exactly.
fn column_pass(v1: V1, coef: &ColumnCoefficients, input: [f32x4; 8]) -> [f32x4; 8] {
    let mul = |a, b| v1.mul_f32x4(a, b);
    let add = |a, b| v1.add_f32x4(a, b);
    let sub = |a, b| v1.sub_f32x4(a, b);

    let (in0, in1, in2, in3, in4, in5, in6, in7) =
        (input[0], input[1], input[2], input[3], input[4], input[5], input[6], input[7]);

    let beta0 =
        add(add(mul(in1, coef.b), mul(in3, coef.d)), add(mul(in5, coef.e), mul(in7, coef.g)));
    let beta1 =
        sub(sub(mul(in1, coef.d), mul(in3, coef.g)), add(mul(in5, coef.b), mul(in7, coef.e)));
    let beta2 =
        add(sub(mul(in1, coef.e), mul(in3, coef.b)), add(mul(in5, coef.g), mul(in7, coef.d)));
    let beta3 =
        add(sub(mul(in1, coef.g), mul(in3, coef.e)), sub(mul(in5, coef.d), mul(in7, coef.b)));

    let theta0 = mul(coef.a, add(in0, in4));
    let theta3 = mul(coef.a, sub(in0, in4));

    let alpha0 = mul(coef.c, in2);
    let alpha1 = mul(coef.f, in2);
    let alpha2 = mul(coef.c, in6);
    let alpha3 = mul(coef.f, in6);

    let theta1 = add(alpha0, alpha3);
    let theta2 = sub(alpha1, alpha2);

    let gamma0 = add(theta0, theta1);
    let gamma1 = add(theta3, theta2);
    let gamma2 = sub(theta3, theta2);
    let gamma3 = sub(theta0, theta1);

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

pub fn dct_inverse_8x8(v1: V1, data: &mut [f32; 64]) {
    let row_coef = RowCoefficients::new();
    for row in 0..8 {
        let base = row * 8;
        let (lo, hi) = row_pass(v1, &row_coef, &data[base..base + 8]);
        data[base] = lo.0;
        data[base + 1] = lo.1;
        data[base + 2] = lo.2;
        data[base + 3] = lo.3;
        data[base + 4] = hi.0;
        data[base + 5] = hi.1;
        data[base + 6] = hi.2;
        data[base + 7] = hi.3;
    }

    let col_coef = ColumnCoefficients::new(v1);
    // Two batches of 4 columns each.
    for half in 0..2 {
        let offset = half * 4;
        let input: [f32x4; 8] = std::array::from_fn(|row| {
            let b = row * 8 + offset;
            f32x4(data[b], data[b + 1], data[b + 2], data[b + 3])
        });
        let out = column_pass(v1, &col_coef, input);
        for (row, result) in out.iter().enumerate() {
            let b = row * 8 + offset;
            data[b] = result.0;
            data[b + 1] = result.1;
            data[b + 2] = result.2;
            data[b + 3] = result.3;
        }
    }
}

struct ForwardCoefficients {
    low: [f32x4; 8],
    high: [f32x4; 8],
}

impl ForwardCoefficients {
    #[inline(always)]
    fn new(_v1: V1) -> Self {
        let basis = forward_basis();
        Self {
            low: std::array::from_fn(|input| {
                let row = basis[input];
                f32x4(row[0], row[1], row[2], row[3])
            }),
            high: std::array::from_fn(|input| {
                let row = basis[input];
                f32x4(row[4], row[5], row[6], row[7])
            }),
        }
    }
}

#[inline(always)]
fn forward_pass(v1: V1, coef: &[f32x4; 8], input: [f32; 8]) -> f32x4 {
    let mul = |a, b| v1.mul_f32x4(a, b);
    let add = |a, b| v1.add_f32x4(a, b);
    let splat = |value: f32| v1.splat_f32x4(value);

    let mut out = v1.splat_f32x4(0.0);
    for index in 0..8 {
        out = add(out, mul(splat(input[index]), coef[index]));
    }
    out
}

pub fn dct_forward_8x8(v1: V1, data: &mut [f32; 64]) {
    let coef = ForwardCoefficients::new(v1);

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
        let low = forward_pass(v1, &coef.low, input);
        let high = forward_pass(v1, &coef.high, input);
        data[base] = low.0;
        data[base + 1] = low.1;
        data[base + 2] = low.2;
        data[base + 3] = low.3;
        data[base + 4] = high.0;
        data[base + 5] = high.1;
        data[base + 6] = high.2;
        data[base + 7] = high.3;
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
        let low = forward_pass(v1, &coef.low, input);
        let high = forward_pass(v1, &coef.high, input);
        data[column] = low.0;
        data[8 + column] = low.1;
        data[16 + column] = low.2;
        data[24 + column] = low.3;
        data[32 + column] = high.0;
        data[40 + column] = high.1;
        data[48 + column] = high.2;
        data[56 + column] = high.3;
    }
}
