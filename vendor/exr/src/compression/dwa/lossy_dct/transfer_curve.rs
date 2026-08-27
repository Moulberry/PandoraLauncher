// The DWA perceptual transfer curve and its inverse, plus the two 64K-entry
// half-float lookup tables the encoder and decoder use to apply them: linear
// values are stored nonlinearly before the DCT and converted back afterwards.

use std::sync::OnceLock;

use half::f16;

pub(super) fn to_nonlinear_table() -> &'static [u16; 65536] {
    static TABLE: OnceLock<[u16; 65536]> = OnceLock::new();

    TABLE.get_or_init(|| {
        // Build the forward transfer table lazily so the hot path stays cheap
        // once the table is initialized.
        let mut table = [0u16; 65536];
        for (bits, slot) in table.iter_mut().enumerate() {
            *slot = dwa_convert_to_nonlinear(f16::from_bits(bits as u16)).to_bits();
        }
        table
    })
}

fn dwa_convert_to_nonlinear(x: f16) -> f16 {
    // Inverse of the decoder's nonlinear -> linear transfer.
    // Values <= 1 use a power curve; values above 1 follow the exponential tail.
    let value = x.to_f32();
    if !value.is_finite() {
        return f16::ZERO;
    }

    let sign = if value < 0.0 {
        -1.0
    } else {
        1.0
    };
    let value = value.abs();

    let nonlinear = if value <= 1.0 {
        value.powf(1.0 / 2.2)
    } else {
        1.0 + value.ln() / 9.02501329156_f32.ln()
    };

    f16::from_f32(sign * nonlinear)
}

/// The stored nonlinear --> linear lookup table for all half bit patterns
pub(super) fn to_linear_table() -> &'static [u16; 65536] {
    static TABLE: OnceLock<[u16; 65536]> = OnceLock::new();

    TABLE.get_or_init(|| {
        // Build the nonlinear -> linear transfer table lazily and reuse it
        // across every chunk.
        let mut table = [0u16; 65536];
        for (bits, slot) in table.iter_mut().enumerate() {
            *slot = dwa_convert_to_linear(f16::from_bits(bits as u16)).to_bits();
        }
        table
    })
}

fn dwa_convert_to_linear(x: f16) -> f16 {
    // Inverse of the encoder's nonlinear transfer.
    let value = x.to_f32();
    if !value.is_finite() {
        return f16::ZERO;
    }

    let sign = if value < 0.0 {
        -1.0
    } else {
        1.0
    };
    let value = value.abs();

    let linear = if value <= 1.0 {
        value.powf(2.2)
    } else {
        // exp(2.2) ^ (value - 1) == exp(2.2 * (value - 1))
        (9.02501329156_f32).powf(value - 1.0)
    };

    f16::from_f32(sign * linear)
}

#[cfg(test)]
mod test {
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::image::validate_results::ValidateResult;

    const SEED: [u8; 32] = [
        44, 201, 17, 88, 6, 255, 61, 30, 11, 2, 121, 99, 1, 250, 77, 33, 7, 42, 13, 200, 176, 22,
        5, 66, 100, 19, 240, 8, 91, 3, 128, 9,
    ];

    fn assert_curve_roundtrips(value: f32) {
        let x = f16::from_f32(value);
        let restored = dwa_convert_to_linear(dwa_convert_to_nonlinear(x));
        x.assert_approx_equals_result(&restored);
    }

    /// Applying the forward transfer curve and then its inverse must recover
    /// the original value (approximately; the curves round-trip through f16).
    /// Restricted to a moderate magnitude range to avoid the f16-coarseness of
    /// the exponential tail at extreme values.
    #[test]
    fn transfer_curve_roundtrip_scalar() {
        for &value in &[0.0f32, 0.1, 0.25, 0.5, 0.75, 1.0, 2.0, 3.5] {
            assert_curve_roundtrips(value);
            assert_curve_roundtrips(-value);
        }

        let mut random = rand::rngs::StdRng::from_seed(SEED);
        for _ in 0..512 {
            assert_curve_roundtrips(random.gen_range(-4.0f32..4.0));
        }
    }

    /// The two 64K lookup tables are the tabulated forward/inverse curves, so
    /// composing them must be approximately the identity over every finite,
    /// moderate-magnitude half-float bit pattern.
    #[test]
    fn transfer_curve_roundtrip_tables() {
        let to_nonlinear = to_nonlinear_table();
        let to_linear = to_linear_table();

        for bits in 0..=u16::MAX {
            let value = f16::from_bits(bits);
            let magnitude = value.to_f32().abs();
            if !value.to_f32().is_finite() || magnitude > 4.0 {
                continue;
            }

            let restored = f16::from_bits(to_linear[to_nonlinear[bits as usize] as usize]);
            value.assert_approx_equals_result(&restored);
        }
    }
}
