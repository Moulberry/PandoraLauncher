use half::f16;
use smallvec::smallvec;

use super::*;
use crate::{
    math::Vec2,
    meta::attribute::{ChannelDescription, Text},
};

fn bounds(width: usize, height: usize) -> IntegerBounds {
    IntegerBounds::new(Vec2(0, 0), Vec2(width, height))
}

#[test]
fn compress_decompress_rle_only_is_lossless() {
    let channels = ChannelList::new(smallvec![ChannelDescription::named("A", SampleType::F16)]);
    let rectangle = bounds(17, 3);
    let mut raw = Vec::new();

    for y in 0..rectangle.size.height() {
        for x in 0..rectangle.size.width() {
            let value = if (x + y) % 3 == 0 {
                f16::from_f32(0.25)
            } else {
                f16::from_f32(0.75)
            };
            raw.extend_from_slice(&value.to_bits().to_ne_bytes());
        }
    }

    let compressed = compress(&channels, raw.clone(), rectangle, Some(45.0)).unwrap();
    let decoded = decompress(&channels, compressed, rectangle, raw.len(), true).unwrap();

    assert_eq!(decoded, raw);
}

#[test]
fn compress_decompress_rgb_lossy_chunk_is_valid() {
    let channels = ChannelList::new(smallvec![
        ChannelDescription::named(Text::from("B"), SampleType::F16),
        ChannelDescription::named(Text::from("G"), SampleType::F16),
        ChannelDescription::named(Text::from("R"), SampleType::F16),
    ]);
    let rectangle = bounds(9, 9);
    let mut raw = Vec::new();

    for y in 0..rectangle.size.height() {
        for channel in 0..3 {
            for x in 0..rectangle.size.width() {
                let value = (x as f32 * 0.03125) + (y as f32 * 0.015625) + channel as f32 * 0.1;
                raw.extend_from_slice(&f16::from_f32(value).to_bits().to_ne_bytes());
            }
        }
    }

    let compressed = compress(&channels, raw.clone(), rectangle, Some(45.0)).unwrap();
    assert_ne!(compressed.len(), raw.len());
    let decoded = decompress(&channels, compressed, rectangle, raw.len(), true).unwrap();

    assert_eq!(decoded.len(), raw.len());
    assert!(decoded.iter().any(|&byte| byte != 0));
}
