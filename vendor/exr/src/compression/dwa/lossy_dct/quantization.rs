// Coefficient (de)quantization for the lossy DCT: the JPEG-derived tolerance
// tables, the quantize-and-scatter-to-zig-zag encode step, the inverse zig-zag
// gather, and the AC run-length (de)coding. The zig-zag scatter (`INV_REMAP`)
// and gather (`SRC_INDICES`) permutations are exact inverses of each other.

use half::f16;

use super::{half_float_quantizer::algo_quantize, PackedStream};
use crate::error::{Error, Result};

pub(super) struct QuantTables {
    pub(super) y: [f32; 64],
    pub(super) half_y: [u16; 64],
    pub(super) cbcr: [f32; 64],
    pub(super) half_cbcr: [u16; 64],
}

impl QuantTables {
    pub(super) fn new(quant_base_error: f32) -> Self {
        // JPEG-style tables, normalized by their minimum entry and scaled by
        // the configured DWA base error.
        const JPEG_Y: [i32; 64] = [
            16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57,
            69, 56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55,
            64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100,
            103, 99,
        ];
        const JPEG_CBCR: [i32; 64] = [
            17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99,
            99, 99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
            99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
        ];

        let quant_base_error = quant_base_error.max(0.0);
        let mut y = [0.0; 64];
        let mut half_y = [0; 64];
        let mut cbcr = [0.0; 64];
        let mut half_cbcr = [0; 64];

        for index in 0..64 {
            y[index] = quant_base_error * JPEG_Y[index] as f32 / 10.0;
            half_y[index] = f16::from_f32(y[index]).to_bits();
            cbcr[index] = quant_base_error * JPEG_CBCR[index] as f32 / 17.0;
            half_cbcr[index] = f16::from_f32(cbcr[index]).to_bits();
        }

        Self {
            y,
            half_y,
            cbcr,
            half_cbcr,
        }
    }
}

pub(super) fn quantize_coefficients_to_zigzag(
    dct_values: &[f32; 64],
    tolerances: &[f32; 64],
    half_tolerances: &[u16; 64],
) -> [u16; 64] {
    // Quantize in DCT order, then scatter into the stored zig-zag layout.
    const INV_REMAP: [usize; 64] = [
        0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9,
        11, 18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60,
        21, 34, 37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
    ];

    let mut half_zig = [0u16; 64];
    for i in 0..64 {
        let src = f16::from_f32(dct_values[i]).to_bits();
        let quantized = algo_quantize(
            src as u32,
            half_tolerances[i] as u32,
            tolerances[i],
            f16::from_bits(src).to_f32(),
        );
        half_zig[INV_REMAP[i]] = quantized as u16;
    }
    half_zig
}

pub(super) fn rle_ac(block: &[u16; 64], ac: &mut Vec<u16>) {
    // The AC stream uses a simple token format: literals are emitted as-is,
    // and runs of zeroes are encoded as 0xffxx tokens. 0xff00 marks EOB.
    let mut dct_comp = 1;

    while dct_comp < 64 {
        if block[dct_comp] != 0 {
            ac.push(block[dct_comp]);
            dct_comp += 1;
            continue;
        }

        let mut run_len = 1;
        while dct_comp + run_len < 64 && block[dct_comp + run_len] == 0 {
            run_len += 1;
        }

        if run_len == 1 {
            ac.push(block[dct_comp]);
        } else if run_len + dct_comp == 64 {
            ac.push(0xff00);
        } else {
            ac.push(0xff00 | run_len as u16);
        }

        dct_comp += run_len;
    }
}

/// Un-RLE one 8x8 block of AC values into block[1..]
/// (`LossyDctDecoder_unRleAc`): a value with high byte 0xff encodes a run
/// of `low byte` zeros (0 meaning "rest of the block"); anything else is a
/// literal. Returns the index of the last non-zero value, 0 if none
pub(super) fn un_rle_ac(ac: &mut PackedStream<'_>, block: &mut [u16; 64]) -> Result<usize> {
    // DWA AC values use the same compact token format the encoder writes:
    // 0xffxx means a zero run, and 0xff00 means end-of-block.
    let mut last_non_zero = 0;
    let mut position = 1;

    while position < 64 {
        let value = ac.next().ok_or_else(|| Error::invalid("truncated DWA AC data"))?;

        if (value & 0xff00) == 0xff00 {
            // run of zeros - the block is pre-zeroed, just skip ahead
            let count = (value & 0xff) as usize;
            position += if count == 0 {
                64
            } else {
                count
            };
        } else {
            last_non_zero = position;
            block[position] = value;
            position += 1;
        }
    }

    Ok(last_non_zero)
}

/// Undo the zig-zag coefficient order (C "fromHalfZigZag_scalar"),
/// converting half bits to f32.
pub(super) fn from_half_zigzag(zig_zag: &[u16; 64], dst: &mut [f32; 64]) {
    // The encoder stores coefficients in zig-zag order; the inverse DCT needs
    // normal 8x8 raster order.
    const SRC_INDICES: [usize; 64] = [
        0, 1, 5, 6, 14, 15, 27, 28, 2, 4, 7, 13, 16, 26, 29, 42, 3, 8, 12, 17, 25, 30, 41, 43, 9,
        11, 18, 24, 31, 40, 44, 53, 10, 19, 23, 32, 39, 45, 52, 54, 20, 22, 33, 38, 46, 51, 55, 60,
        21, 34, 37, 47, 50, 56, 59, 61, 35, 36, 48, 49, 57, 58, 62, 63,
    ];

    for (slot, &src_index) in dst.iter_mut().zip(SRC_INDICES.iter()) {
        *slot = f16::from_bits(zig_zag[src_index]).to_f32();
    }
}

// Note: `quantize_coefficients_to_zigzag`/`from_half_zigzag` are intentionally
// not roundtrip-tested for value equality. The zig-zag scatter/gather
// permutations are exact inverses, but `quantize_coefficients_to_zigzag` also
// lossily quantizes the coefficients, so `from_half_zigzag(quantize(x)) != x`
// by design. The lossless half of quantization is the AC run-length coding
// below, which is what these tests cover.
#[cfg(test)]
mod test {
    use rand::{Rng, SeedableRng};

    use super::*;

    const SEED: [u8; 32] = [
        250, 77, 33, 7, 42, 13, 200, 176, 22, 5, 66, 100, 19, 240, 8, 91, 3, 128, 9, 44, 201, 17,
        88, 6, 255, 61, 30, 11, 2, 121, 99, 1,
    ];

    /// Run-length-encode an AC block, decode it back, and require the AC
    /// coefficients (indices 1..64) to be recovered exactly. Index 0 (DC) is
    /// not part of the AC stream, so it is kept zero on both sides.
    fn assert_ac_roundtrips(block: [u16; 64]) {
        let mut ac = Vec::new();
        rle_ac(&block, &mut ac);

        let mut stream = PackedStream::new(&ac);
        let mut decoded = [0u16; 64];
        un_rle_ac(&mut stream, &mut decoded).unwrap();

        assert_eq!(decoded, block);
    }

    #[test]
    fn ac_run_length_roundtrip_hardcoded() {
        // All-zero AC (immediate end-of-block).
        assert_ac_roundtrips([0u16; 64]);

        // No zeros at all: every AC coefficient is a literal.
        let mut dense = [0u16; 64];
        for (index, slot) in dense.iter_mut().enumerate().skip(1) {
            *slot = index as u16;
        }
        assert_ac_roundtrips(dense);

        // A mix of literals, an interior zero run, a single isolated zero, and
        // a trailing zero run that ends the block.
        let mut mixed = [0u16; 64];
        mixed[1] = 5;
        // mixed[2..10] stay zero -> interior run
        mixed[10] = 7;
        mixed[11] = 0; // isolated single zero
        mixed[12] = 9;
        // mixed[13..64] stay zero -> trailing run to end
        assert_ac_roundtrips(mixed);
    }

    #[test]
    fn ac_run_length_roundtrip_seeded() {
        let mut random = rand::rngs::StdRng::from_seed(SEED);

        for _ in 0..64 {
            let mut block = [0u16; 64];
            for slot in block.iter_mut().skip(1) {
                // ~30% zeros to exercise runs; non-zero literals must stay out
                // of the 0xff00..=0xffff token range the format reserves for
                // zero-run markers.
                *slot = if random.gen_bool(0.3) {
                    0
                } else {
                    random.gen_range(1..=0xfeff)
                };
            }
            assert_ac_roundtrips(block);
        }
    }
}
