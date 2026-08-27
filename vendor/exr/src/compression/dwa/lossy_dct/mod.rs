// The lossy DCT codec: encoding and decoding the shared AC/DC coefficient
// streams that every LOSSY_DCT channel group of a chunk consumes. Includes the
// Y'CbCr <-> R'G'B' color-space conversion the RGB triplets are transformed
// with (the modified 709 coefficients from OpenEXRCore internal_dwa_simd.h).

use half::f16;

use super::{discrete_cosine_transform, ChannelInfo, CompressorScheme};
use crate::{
    error::{Error, Result},
    meta::attribute::SampleType,
};

mod half_float_quantizer;
mod quantization;
mod transfer_curve;

use quantization::{
    from_half_zigzag, quantize_coefficients_to_zigzag, rle_ac, un_rle_ac, QuantTables,
};
use transfer_curve::{to_linear_table, to_nonlinear_table};

/// Y'CbCr -> R'G'B' inverse conversion for DWA, using the modified 709
/// coefficients OpenEXR's DWA encoder uses. Input comp0/1/2 are Y, RY, BY;
/// output is R, G, B.
#[inline]
fn csc709_inverse(comp0: f32, comp1: f32, comp2: f32) -> (f32, f32, f32) {
    let r = comp0 + 1.5747 * comp2;
    let g = comp0 - 0.1873 * comp1 - 0.4682 * comp2;
    let b = comp0 + 1.8556 * comp1;
    (r, g, b)
}

/// R'G'B' -> Y'CbCr forward conversion for DWA. The component order matches
/// OpenEXR's channel-group storage: Y, BY, RY.
#[inline]
fn csc709_forward(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // OpenEXR uses a modified 709 transform with a zero-centered chroma
    // representation instead of the usual 0.5 offset.
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let by = (b - y) / 1.8556;
    let ry = (r - y) / 1.5747;
    (y, by, ry)
}

pub(super) fn encode_lossy_channels(
    infos: &[ChannelInfo],
    csc_groups: &[[usize; 3]],
    channel_bytes: &[Vec<u8>],
    quant_base_error: f32,
) -> Result<(Vec<u16>, Vec<u16>)> {
    // Lossy chunks use shared AC/DC streams. CSC triplets consume the streams
    // first, then standalone LOSSY_DCT channels continue from the same cursors.
    let mut ac = Vec::new();
    let mut dc = Vec::new();
    let mut grouped = vec![false; infos.len()];

    for &group in csc_groups {
        let info = &infos[group[0]];
        let components = group
            .iter()
            .map(|&channel| channel_half_samples(&channel_bytes[channel], &infos[channel], true))
            .collect::<Result<Vec<_>>>()?;

        encode_lossy_dct_group(
            &components,
            info.width,
            info.height,
            quant_base_error,
            &mut ac,
            &mut dc,
        )?;

        for &channel in &group {
            grouped[channel] = true;
        }
    }

    for (index, info) in infos.iter().enumerate() {
        if grouped[index] || info.scheme != CompressorScheme::LossyDct {
            continue;
        }

        let apply_nonlinear = !info.quantize_linearly;
        let samples = channel_half_samples(&channel_bytes[index], info, apply_nonlinear)?;
        encode_lossy_dct_group(
            &[samples],
            info.width,
            info.height,
            quant_base_error,
            &mut ac,
            &mut dc,
        )?;
    }

    Ok((ac, dc))
}

fn channel_half_samples(
    bytes: &[u8],
    info: &ChannelInfo,
    apply_nonlinear: bool,
) -> Result<Vec<u16>> {
    // OpenEXR stores lossy input as half precision internally before DCT.
    // F32 channels are clamped to the finite half range and demoted here.
    let mut samples = Vec::with_capacity(info.width * info.height);

    match info.sample_type {
        SampleType::F16 => {
            let chunks = bytes.chunks_exact(2);
            if !chunks.remainder().is_empty() {
                return Err(Error::invalid("DWA f16 channel data size"));
            }
            samples.extend(chunks.map(|pair| u16::from_le_bytes([pair[0], pair[1]])));
        }
        SampleType::F32 => {
            let chunks = bytes.chunks_exact(4);
            if !chunks.remainder().is_empty() {
                return Err(Error::invalid("DWA f32 channel data size"));
            }
            samples.extend(chunks.map(|quad| {
                let mut value = f32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]);
                value = value.clamp(-65504.0, 65504.0);
                f16::from_f32(value).to_bits()
            }));
        }
        SampleType::U32 => {
            return Err(Error::unsupported("DWA lossy DCT compression of u32 channels"));
        }
    }

    if samples.len() != info.width * info.height {
        return Err(Error::invalid("DWA lossy channel data size mismatch"));
    }

    if apply_nonlinear {
        let to_nonlinear = to_nonlinear_table();
        for sample in &mut samples {
            *sample = to_nonlinear[*sample as usize];
        }
    }

    Ok(samples)
}

fn encode_lossy_dct_group(
    components: &[Vec<u16>],
    width: usize,
    height: usize,
    quant_base_error: f32,
    ac: &mut Vec<u16>,
    dc: &mut Vec<u16>,
) -> Result<()> {
    if width == 0 || height == 0 {
        return Ok(());
    }

    let component_count = components.len();
    if component_count != 1 && component_count != 3 {
        return Err(Error::invalid("invalid DWA lossy component count"));
    }

    for component in components {
        if component.len() != width * height {
            return Err(Error::invalid("DWA lossy component size mismatch"));
        }
    }

    // Mirror the source block edges so partial 8x8 blocks behave the same way
    // as the reference encoder.
    let quant_tables = QuantTables::new(quant_base_error);
    let blocks_x = (width + 7) / 8;
    let blocks_y = (height + 7) / 8;
    let block_count = blocks_x * blocks_y;
    let mut group_dc: Vec<Vec<u16>> =
        (0..component_count).map(|_| Vec::with_capacity(block_count)).collect();

    let mut row_blocks: Vec<[[f32; 64]; 3]> = vec![[[0.0; 64]; 3]; blocks_x];

    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            for component_index in 0..component_count {
                let block = &mut row_blocks[block_x][component_index];
                for y in 0..8 {
                    let src_y = mirror_index(block_y * 8 + y, height);
                    for x in 0..8 {
                        let src_x = mirror_index(block_x * 8 + x, width);
                        let bits = components[component_index][src_y * width + src_x];
                        block[y * 8 + x] = f16::from_bits(bits).to_f32();
                    }
                }
            }

            if component_count == 3 {
                // CSC is performed in nonlinear space for the RGB triplet.
                let dct_blocks = &mut row_blocks[block_x];
                for i in 0..64 {
                    let (y, by, ry) =
                        csc709_forward(dct_blocks[0][i], dct_blocks[1][i], dct_blocks[2][i]);
                    dct_blocks[0][i] = y;
                    dct_blocks[1][i] = by;
                    dct_blocks[2][i] = ry;
                }
            }
        }

        discrete_cosine_transform::dct_forward_8x8_batch(
            row_blocks.iter_mut().flat_map(|blocks| blocks[..component_count].iter_mut()),
        );

        for block_x in 0..blocks_x {
            for component_index in 0..component_count {
                let block = &mut row_blocks[block_x][component_index];
                let (tolerances, half_tolerances) = if component_index == 0 {
                    (&quant_tables.y, &quant_tables.half_y)
                } else {
                    (&quant_tables.cbcr, &quant_tables.half_cbcr)
                };

                let half_zig = quantize_coefficients_to_zigzag(block, tolerances, half_tolerances);
                group_dc[component_index].push(half_zig[0]);
                rle_ac(&half_zig, ac);
            }
        }
    }

    for component_dc in group_dc {
        dc.extend(component_dc);
    }

    Ok(())
}

fn mirror_index(index: usize, length: usize) -> usize {
    // The C encoder mirrors out-of-bounds coordinates back into the image
    // rather than clamping them. This keeps edge blocks symmetrical.
    debug_assert_ne!(length, 0);
    let mut value = index as isize;
    let length = length as isize;

    if value >= length {
        value = length - (value - (length - 1));
    }
    if value < 0 {
        value = length - 1;
    }

    value as usize
}

/// One of the chunk-global u16 streams (AC or DC). All channel groups of a
/// chunk consume the same stream, so the cursor carries across groups.
pub(super) struct PackedStream<'v> {
    values: &'v [u16],
    cursor: usize,
}

impl<'v> PackedStream<'v> {
    fn new(values: &'v [u16]) -> Self {
        Self {
            values,
            cursor: 0,
        }
    }

    fn next(&mut self) -> Option<u16> {
        let value = self.values.get(self.cursor).copied();
        self.cursor += 1;
        value
    }

    /// Value at "offset" past the cursor, without consuming (the DC stream
    /// is indexed planar per group and advanced once at group end).
    fn peek_at(&self, offset: usize) -> Option<u16> {
        self.values.get(self.cursor + offset).copied()
    }

    fn advance(&mut self, count: usize) {
        self.cursor += count;
    }
}

/// Decode all LOSSY_DCT channels: first every CSC group, then the
/// standalone channels, both in channel order - the order in which the
/// encoder appended them to the shared AC/DC streams.
pub(super) fn decode_lossy_channels(
    infos: &[ChannelInfo],
    csc_groups: &[[usize; 3]],
    ac_packed: &[u16],
    dc_packed: &[u16],
) -> Result<Vec<Vec<f16>>> {
    // Decode CSC triplets first, then standalone lossy channels. The shared
    // AC/DC cursors advance in the same order the encoder wrote them.
    let mut ac = PackedStream::new(ac_packed);
    let mut dc = PackedStream::new(dc_packed);

    let mut samples: Vec<Vec<f16>> = vec![vec![]; infos.len()];
    let mut grouped = vec![false; infos.len()];

    for &group in csc_groups {
        // all three channels have identical sampling, hence identical size
        let info = &infos[group[0]];
        let mut decoded: [Vec<f16>; 3] =
            std::array::from_fn(|_| vec![f16::ZERO; info.width * info.height]);

        decode_lossy_dct_group(
            &mut ac,
            &mut dc,
            info.width,
            info.height,
            Some(to_linear_table()),
            &mut decoded,
        )?;

        for (&channel, channel_samples) in group.iter().zip(decoded) {
            samples[channel] = channel_samples;
            grouped[channel] = true;
        }
    }

    for (index, info) in infos.iter().enumerate() {
        if grouped[index] || info.scheme != CompressorScheme::LossyDct {
            continue;
        }
        let mut decoded = [vec![f16::ZERO; info.width * info.height]];
        let to_linear = (!info.quantize_linearly).then(to_linear_table);
        decode_lossy_dct_group(&mut ac, &mut dc, info.width, info.height, to_linear, &mut decoded)?;

        let [channel_samples] = decoded;
        samples[index] = channel_samples;
    }

    Ok(samples)
}

/// Decode one standalone channel (decoded.len() == 1) or one CSC'd R/G/B
/// triplet (decoded.len() == 3): per 8x8 block and component, read the
/// DC value, un-RLE the AC values, inverse-DCT
fn decode_lossy_dct_group(
    ac: &mut PackedStream<'_>,
    dc: &mut PackedStream<'_>,
    width: usize,
    height: usize,
    to_linear: Option<&[u16; 65536]>,
    decoded: &mut [Vec<f16>],
) -> Result<()> {
    let components = decoded.len();
    let blocks_x = (width + 7) / 8;
    let blocks_y = (height + 7) / 8;
    let block_count = blocks_x * blocks_y;

    // Buffer the whole group rather than one block at a time. That lets the
    // inverse DCT batch over every block that actually needs it, matching the
    // structure of the C reference.
    let mut dct_blocks: Vec<[[f32; 64]; 3]> = vec![[[0.0f32; 64]; 3]; block_count];
    let mut needs_inverse_dct: Vec<[bool; 3]> = vec![[false; 3]; block_count];

    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let block_index = block_y * blocks_x + block_x;

            for component in 0..components {
                let mut zig_block = [0u16; 64];

                // the DC stream is planar: all of component 0's blocks,
                // then all of component 1's, ...
                zig_block[0] = dc
                    .peek_at(component * block_count + block_index)
                    .ok_or_else(|| Error::invalid("truncated DWA DC data"))?;

                let last_non_zero = un_rle_ac(ac, &mut zig_block)?;

                let dct_block = &mut dct_blocks[block_index][component];
                if last_non_zero == 0 {
                    // DC-only block: all AC coefficients are zero, so the
                    // inverse DCT can fill the whole block from one value.
                    dct_block[0] = f16::from_bits(zig_block[0]).to_f32();
                    discrete_cosine_transform::dct_inverse_8x8_dc_only(dct_block);
                } else {
                    from_half_zigzag(&zig_block, dct_block);
                    needs_inverse_dct[block_index][component] = true;
                }
            }
        }
    }

    discrete_cosine_transform::dct_inverse_8x8_batch(
        dct_blocks
            .iter_mut()
            .zip(needs_inverse_dct.iter())
            .flat_map(|(blocks, flags)| blocks.iter_mut().zip(flags.iter()))
            .filter_map(|(block, &needed)| needed.then_some(block)),
    );

    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let block_index = block_y * blocks_x + block_x;
            let dct_blocks = &mut dct_blocks[block_index];

            if components == 3 {
                for i in 0..64 {
                    let (r, g, b) =
                        csc709_inverse(dct_blocks[0][i], dct_blocks[1][i], dct_blocks[2][i]);
                    dct_blocks[0][i] = r;
                    dct_blocks[1][i] = g;
                    dct_blocks[2][i] = b;
                }
            }

            // Convert nonlinear DCT output back to linear half values and crop
            // the edges to the actual image extent.
            for (component, output) in decoded.iter_mut().enumerate() {
                for y in block_y * 8..(block_y * 8 + 8).min(height) {
                    for x in block_x * 8..(block_x * 8 + 8).min(width) {
                        let value =
                            dct_blocks[component][(y - block_y * 8) * 8 + (x - block_x * 8)];
                        let nonlinear = f16::from_f32(value);
                        output[y * width + x] = f16::from_bits(match to_linear {
                            Some(to_linear) => to_linear[nonlinear.to_bits() as usize],
                            None => nonlinear.to_bits(),
                        });
                    }
                }
            }
        }
    }

    dc.advance(components * block_count);
    Ok(())
}

#[cfg(test)]
mod test {
    use rand::{Rng, SeedableRng};

    use super::*;
    use crate::image::validate_results::ValidateResult;

    const SEED: [u8; 32] = [
        66, 100, 19, 240, 8, 91, 3, 128, 9, 44, 201, 17, 88, 6, 255, 61, 30, 11, 2, 121, 99, 1,
        250, 77, 33, 7, 42, 13, 200, 176, 22, 5,
    ];

    /// The R'G'B' <-> Y'CbCr conversion pair must round-trip: converting to
    /// Y'CbCr and back must recover the original RGB triple (approximately,
    /// since the matrix coefficients are not exactly invertible in f32). The
    /// forward output tuple `(y, by, ry)` feeds the inverse positionally.
    fn assert_csc_roundtrips(r: f32, g: f32, b: f32) {
        let (y, by, ry) = csc709_forward(r, g, b);
        let (r2, g2, b2) = csc709_inverse(y, by, ry);
        vec![r, g, b].assert_approx_equals_result(&vec![r2, g2, b2]);
    }

    #[test]
    fn csc_roundtrip_hardcoded() {
        assert_csc_roundtrips(0.0, 0.0, 0.0);
        assert_csc_roundtrips(1.0, 1.0, 1.0);
        assert_csc_roundtrips(1.0, 0.0, 0.0);
        assert_csc_roundtrips(0.0, 1.0, 0.0);
        assert_csc_roundtrips(0.0, 0.0, 1.0);
        assert_csc_roundtrips(0.25, 0.5, 0.75);
    }

    #[test]
    fn csc_roundtrip_seeded() {
        let mut random = rand::rngs::StdRng::from_seed(SEED);
        for _ in 0..256 {
            let mut channel = || random.gen_range(-4.0f32..4.0);
            assert_csc_roundtrips(channel(), channel(), channel());
        }
    }
}
