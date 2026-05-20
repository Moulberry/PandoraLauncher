use std::{
    io::{Read, Write},
    path::PathBuf,
};

use ppmd_rust::{Ppmd7Decoder, Ppmd7Encoder, Ppmd8Decoder, Ppmd8Encoder, RestoreMethod};

trait PPMdVersion {
    fn compression(level: u32) -> (u32, u32);
    fn data_path(bin_directory: &str, file_stem: &str, level: u32) -> PathBuf;
    fn compress(text: &str, order: u32, memory_size: u32) -> Vec<u8>;
    fn decompress(
        compressed: &[u8],
        uncompressed_size: usize,
        order: u32,
        memory_size: u32,
    ) -> String;
}

struct PPMd7;

impl PPMdVersion for PPMd7 {
    fn compression(level: u32) -> (u32, u32) {
        const ORDERS: [u32; 10] = [3, 4, 4, 5, 5, 6, 8, 16, 24, 32];
        let order = ORDERS[level as usize];
        let memory_size = 1 << (level + 19);
        (order, memory_size)
    }

    fn data_path(bin_directory: &str, file_stem: &str, level: u32) -> PathBuf {
        PathBuf::from(bin_directory).join(format!("{file_stem}_ppmd7_level{level}.bin"))
    }

    fn compress(text: &str, order: u32, memory_size: u32) -> Vec<u8> {
        let mut compressed = Vec::new();
        {
            let mut ppmd7 = Ppmd7Encoder::new(&mut compressed, order, memory_size).unwrap();
            ppmd7.write_all(text.as_bytes()).unwrap();
            ppmd7.finish(false).unwrap();
        }
        compressed
    }

    fn decompress(
        compressed: &[u8],
        uncompressed_size: usize,
        order: u32,
        memory_size: u32,
    ) -> String {
        let mut data = vec![0; uncompressed_size];
        {
            let mut ppmd7 = Ppmd7Decoder::new(compressed, order, memory_size).unwrap();
            ppmd7.read_exact(&mut data).unwrap();
        }
        String::from_utf8(data).unwrap()
    }
}

struct PPMd8;

impl PPMdVersion for PPMd8 {
    fn compression(level: u32) -> (u32, u32) {
        let level = if level > 9 { 9 } else { level };
        let order = 3 + level;
        let memory_size_mb = 1 << (level - 1);
        let memory_size = memory_size_mb << 20;
        (order, memory_size)
    }

    fn data_path(bin_directory: &str, file_stem: &str, level: u32) -> PathBuf {
        PathBuf::from(bin_directory).join(format!("{file_stem}_ppmd8_level{level}.bin"))
    }

    fn compress(text: &str, order: u32, memory_size: u32) -> Vec<u8> {
        let mut compressed = Vec::new();
        {
            let mut ppmd8 =
                Ppmd8Encoder::new(&mut compressed, order, memory_size, RestoreMethod::Restart)
                    .unwrap();
            ppmd8.write_all(text.as_bytes()).unwrap();
            ppmd8.finish(false).unwrap();
        }
        compressed
    }

    fn decompress(
        compressed: &[u8],
        uncompressed_size: usize,
        order: u32,
        memory_size: u32,
    ) -> String {
        {
            let mut data = vec![0; uncompressed_size];
            {
                let mut ppmd8 =
                    Ppmd8Decoder::new(compressed, order, memory_size, RestoreMethod::Restart)
                        .unwrap();
                ppmd8.read_exact(&mut data).unwrap();
            }
            String::from_utf8(data).unwrap()
        }
    }
}

const BIN_DIRECTORY: &str = "tests/fixtures/";
const APACHE2_PATH: &str = "tests/fixtures/text/apache2.txt";
const GPL3_PATH: &str = "tests/fixtures/text/gpl3.txt";
const ISSUE3_PATH: &str = "tests/fixtures/text/issue_3.txt";
const PG100_PATH: &str = "tests/fixtures/text/pg100.txt";
const PG6800_PATH: &str = "tests/fixtures/text/pg6800.txt";

fn test_compression<P: PPMdVersion>(path: &str, level: u32) {
    let path = PathBuf::from(path);
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let original_text = std::fs::read_to_string(&path).unwrap();

    let (order, memory) = P::compression(level);
    let data_path = P::data_path(BIN_DIRECTORY, file_stem, level);

    let new_data = P::compress(original_text.as_str(), order, memory);
    let original_data = std::fs::read(data_path).unwrap();

    // No assert because of performance reason.
    if original_data.as_slice() != new_data.as_slice() {
        panic!("compressed data doesn't match original");
    };
}

fn test_decompression<P: PPMdVersion>(path: &str, level: u32) {
    let path = PathBuf::from(path);
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let original_text = std::fs::read_to_string(&path).unwrap();

    let (order, memory) = P::compression(level);
    let data_path = P::data_path(BIN_DIRECTORY, file_stem, level);

    let original_data = std::fs::read(data_path).unwrap();
    let new_text = P::decompress(original_data.as_slice(), original_text.len(), order, memory);

    // No assert because of performance reason.
    if original_text.as_str() != new_text.as_str() {
        panic!("decompressed text doesn't match original");
    }
}

#[rustfmt::skip]
mod tests {
    use super::*;

    // PPMd7 Compression Tests - Level 1
    #[test] fn compression_ppmd7_apache2_1() { test_compression::<PPMd7>(APACHE2_PATH, 1); }
    #[test] fn compression_ppmd7_gpl3_1() { test_compression::<PPMd7>(GPL3_PATH, 1); }
    #[test] fn compression_ppmd7_issue3_1() { test_compression::<PPMd7>(ISSUE3_PATH, 1); }
    #[test] fn compression_ppmd7_pg100_1() { test_compression::<PPMd7>(PG100_PATH, 1); }
    #[test] fn compression_ppmd7_pg6800_1() { test_compression::<PPMd7>(PG6800_PATH, 1); }

    // PPMd7 Decompression Tests - Level 1
    #[test] fn decompression_ppmd7_apache2_1() { test_decompression::<PPMd7>(APACHE2_PATH, 1); }
    #[test] fn decompression_ppmd7_gpl3_1() { test_decompression::<PPMd7>(GPL3_PATH, 1); }
    #[test] fn decompression_ppmd7_issue3_1() { test_decompression::<PPMd7>(ISSUE3_PATH, 1); }
    #[test] fn decompression_ppmd7_pg100_1() { test_decompression::<PPMd7>(PG100_PATH, 1); }
    #[test] fn decompression_ppmd7_pg6800_1() { test_decompression::<PPMd7>(PG6800_PATH, 1); }

    // PPMd8 Compression Tests - Level 1
    #[test] fn compression_ppmd8_apache2_1() { test_compression::<PPMd8>(APACHE2_PATH, 1); }
    #[test] fn compression_ppmd8_gpl3_1() { test_compression::<PPMd8>(GPL3_PATH, 1); }
    #[test] fn compression_ppmd8_issue3_1() { test_compression::<PPMd8>(ISSUE3_PATH, 1); }
    #[test] fn compression_ppmd8_pg100_1() { test_compression::<PPMd8>(PG100_PATH, 1); }
    #[test] fn compression_ppmd8_pg6800_1() { test_compression::<PPMd8>(PG6800_PATH, 1); }

    // PPMd8 Decompression Tests - Level 1
    #[test] fn decompression_ppmd8_apache2_1() { test_decompression::<PPMd8>(APACHE2_PATH, 1); }
    #[test] fn decompression_ppmd8_gpl3_1() { test_decompression::<PPMd8>(GPL3_PATH, 1); }
    #[test] fn decompression_ppmd8_issue3_1() { test_decompression::<PPMd8>(ISSUE3_PATH, 1); }
    #[test] fn decompression_ppmd8_pg100_1() { test_decompression::<PPMd8>(PG100_PATH, 1); }
    #[test] fn decompression_ppmd8_pg6800_1() { test_decompression::<PPMd8>(PG6800_PATH, 1); }

    // PPMd7 Compression Tests - Level 2
    #[test] fn compression_ppmd7_apache2_2() { test_compression::<PPMd7>(APACHE2_PATH, 2); }
    #[test] fn compression_ppmd7_gpl3_2() { test_compression::<PPMd7>(GPL3_PATH, 2); }
    #[test] fn compression_ppmd7_issue3_2() { test_compression::<PPMd7>(ISSUE3_PATH, 2); }
    #[test] fn compression_ppmd7_pg100_2() { test_compression::<PPMd7>(PG100_PATH, 2); }
    #[test] fn compression_ppmd7_pg6800_2() { test_compression::<PPMd7>(PG6800_PATH, 2); }

    // PPMd7 Decompression Tests - Level 2
    #[test] fn decompression_ppmd7_apache2_2() { test_decompression::<PPMd7>(APACHE2_PATH, 2); }
    #[test] fn decompression_ppmd7_gpl3_2() { test_decompression::<PPMd7>(GPL3_PATH, 2); }
    #[test] fn decompression_ppmd7_issue3_2() { test_decompression::<PPMd7>(ISSUE3_PATH, 2); }
    #[test] fn decompression_ppmd7_pg100_2() { test_decompression::<PPMd7>(PG100_PATH, 2); }
    #[test] fn decompression_ppmd7_pg6800_2() { test_decompression::<PPMd7>(PG6800_PATH, 2); }

    // PPMd8 Compression Tests - Level 2
    #[test] fn compression_ppmd8_apache2_2() { test_compression::<PPMd8>(APACHE2_PATH, 2); }
    #[test] fn compression_ppmd8_gpl3_2() { test_compression::<PPMd8>(GPL3_PATH, 2); }
    #[test] fn compression_ppmd8_issue3_2() { test_compression::<PPMd8>(ISSUE3_PATH, 2); }
    #[test] fn compression_ppmd8_pg100_2() { test_compression::<PPMd8>(PG100_PATH, 2); }
    #[test] fn compression_ppmd8_pg6800_2() { test_compression::<PPMd8>(PG6800_PATH, 2); }

    // PPMd8 Decompression Tests - Level 2
    #[test] fn decompression_ppmd8_apache2_2() { test_decompression::<PPMd8>(APACHE2_PATH, 2); }
    #[test] fn decompression_ppmd8_gpl3_2() { test_decompression::<PPMd8>(GPL3_PATH, 2); }
    #[test] fn decompression_ppmd8_issue3_2() { test_decompression::<PPMd8>(ISSUE3_PATH, 2); }
    #[test] fn decompression_ppmd8_pg100_2() { test_decompression::<PPMd8>(PG100_PATH, 2); }
    #[test] fn decompression_ppmd8_pg6800_2() { test_decompression::<PPMd8>(PG6800_PATH, 2); }

    // PPMd7 Compression Tests - Level 3
    #[test] fn compression_ppmd7_apache2_3() { test_compression::<PPMd7>(APACHE2_PATH, 3); }
    #[test] fn compression_ppmd7_gpl3_3() { test_compression::<PPMd7>(GPL3_PATH, 3); }
    #[test] fn compression_ppmd7_issue3_3() { test_compression::<PPMd7>(ISSUE3_PATH, 3); }
    #[test] fn compression_ppmd7_pg100_3() { test_compression::<PPMd7>(PG100_PATH, 3); }
    #[test] fn compression_ppmd7_pg6800_3() { test_compression::<PPMd7>(PG6800_PATH, 3); }

    // PPMd7 Decompression Tests - Level 3
    #[test] fn decompression_ppmd7_apache2_3() { test_decompression::<PPMd7>(APACHE2_PATH, 3); }
    #[test] fn decompression_ppmd7_gpl3_3() { test_decompression::<PPMd7>(GPL3_PATH, 3); }
    #[test] fn decompression_ppmd7_issue3_3() { test_decompression::<PPMd7>(ISSUE3_PATH, 3); }
    #[test] fn decompression_ppmd7_pg100_3() { test_decompression::<PPMd7>(PG100_PATH, 3); }
    #[test] fn decompression_ppmd7_pg6800_3() { test_decompression::<PPMd7>(PG6800_PATH, 3); }

    // PPMd8 Compression Tests - Level 3
    #[test] fn compression_ppmd8_apache2_3() { test_compression::<PPMd8>(APACHE2_PATH, 3); }
    #[test] fn compression_ppmd8_gpl3_3() { test_compression::<PPMd8>(GPL3_PATH, 3); }
    #[test] fn compression_ppmd8_issue3_3() { test_compression::<PPMd8>(ISSUE3_PATH, 3); }
    #[test] fn compression_ppmd8_pg100_3() { test_compression::<PPMd8>(PG100_PATH, 3); }
    #[test] fn compression_ppmd8_pg6800_3() { test_compression::<PPMd8>(PG6800_PATH, 3); }

    // PPMd8 Decompression Tests - Level 3
    #[test] fn decompression_ppmd8_apache2_3() { test_decompression::<PPMd8>(APACHE2_PATH, 3); }
    #[test] fn decompression_ppmd8_gpl3_3() { test_decompression::<PPMd8>(GPL3_PATH, 3); }
    #[test] fn decompression_ppmd8_issue3_3() { test_decompression::<PPMd8>(ISSUE3_PATH, 3); }
    #[test] fn decompression_ppmd8_pg100_3() { test_decompression::<PPMd8>(PG100_PATH, 3); }
    #[test] fn decompression_ppmd8_pg6800_3() { test_decompression::<PPMd8>(PG6800_PATH, 3); }

    // PPMd7 Compression Tests - Level 4
    #[test] fn compression_ppmd7_apache2_4() { test_compression::<PPMd7>(APACHE2_PATH, 4); }
    #[test] fn compression_ppmd7_gpl3_4() { test_compression::<PPMd7>(GPL3_PATH, 4); }
    #[test] fn compression_ppmd7_issue3_4() { test_compression::<PPMd7>(ISSUE3_PATH, 4); }
    #[test] fn compression_ppmd7_pg100_4() { test_compression::<PPMd7>(PG100_PATH, 4); }
    #[test] fn compression_ppmd7_pg6800_4() { test_compression::<PPMd7>(PG6800_PATH, 4); }

    // PPMd7 Decompression Tests - Level 4
    #[test] fn decompression_ppmd7_apache2_4() { test_decompression::<PPMd7>(APACHE2_PATH, 4); }
    #[test] fn decompression_ppmd7_gpl3_4() { test_decompression::<PPMd7>(GPL3_PATH, 4); }
    #[test] fn decompression_ppmd7_issue3_4() { test_decompression::<PPMd7>(ISSUE3_PATH, 4); }
    #[test] fn decompression_ppmd7_pg100_4() { test_decompression::<PPMd7>(PG100_PATH, 4); }
    #[test] fn decompression_ppmd7_pg6800_4() { test_decompression::<PPMd7>(PG6800_PATH, 4); }

    // PPMd8 Compression Tests - Level 4
    #[test] fn compression_ppmd8_apache2_4() { test_compression::<PPMd8>(APACHE2_PATH, 4); }
    #[test] fn compression_ppmd8_gpl3_4() { test_compression::<PPMd8>(GPL3_PATH, 4); }
    #[test] fn compression_ppmd8_issue3_4() { test_compression::<PPMd8>(ISSUE3_PATH, 4); }
    #[test] fn compression_ppmd8_pg100_4() { test_compression::<PPMd8>(PG100_PATH, 4); }
    #[test] fn compression_ppmd8_pg6800_4() { test_compression::<PPMd8>(PG6800_PATH, 4); }

    // PPMd8 Decompression Tests - Level 4
    #[test] fn decompression_ppmd8_apache2_4() { test_decompression::<PPMd8>(APACHE2_PATH, 4); }
    #[test] fn decompression_ppmd8_gpl3_4() { test_decompression::<PPMd8>(GPL3_PATH, 4); }
    #[test] fn decompression_ppmd8_issue3_4() { test_decompression::<PPMd8>(ISSUE3_PATH, 4); }
    #[test] fn decompression_ppmd8_pg100_4() { test_decompression::<PPMd8>(PG100_PATH, 4); }
    #[test] fn decompression_ppmd8_pg6800_4() { test_decompression::<PPMd8>(PG6800_PATH, 4); }

    // PPMd7 Compression Tests - Level 5
    #[test] fn compression_ppmd7_apache2_5() { test_compression::<PPMd7>(APACHE2_PATH, 5); }
    #[test] fn compression_ppmd7_gpl3_5() { test_compression::<PPMd7>(GPL3_PATH, 5); }
    #[test] fn compression_ppmd7_issue3_5() { test_compression::<PPMd7>(ISSUE3_PATH, 5); }
    #[test] fn compression_ppmd7_pg100_5() { test_compression::<PPMd7>(PG100_PATH, 5); }
    #[test] fn compression_ppmd7_pg6800_5() { test_compression::<PPMd7>(PG6800_PATH, 5); }

    // PPMd7 Decompression Tests - Level 5
    #[test] fn decompression_ppmd7_apache2_5() { test_decompression::<PPMd7>(APACHE2_PATH, 5); }
    #[test] fn decompression_ppmd7_gpl3_5() { test_decompression::<PPMd7>(GPL3_PATH, 5); }
    #[test] fn decompression_ppmd7_issue3_5() { test_decompression::<PPMd7>(ISSUE3_PATH, 5); }
    #[test] fn decompression_ppmd7_pg100_5() { test_decompression::<PPMd7>(PG100_PATH, 5); }
    #[test] fn decompression_ppmd7_pg6800_5() { test_decompression::<PPMd7>(PG6800_PATH, 5); }

    // PPMd8 Compression Tests - Level 5
    #[test] fn compression_ppmd8_apache2_5() { test_compression::<PPMd8>(APACHE2_PATH, 5); }
    #[test] fn compression_ppmd8_gpl3_5() { test_compression::<PPMd8>(GPL3_PATH, 5); }
    #[test] fn compression_ppmd8_issue3_5() { test_compression::<PPMd8>(ISSUE3_PATH, 5); }
    #[test] fn compression_ppmd8_pg100_5() { test_compression::<PPMd8>(PG100_PATH, 5); }
    #[test] fn compression_ppmd8_pg6800_5() { test_compression::<PPMd8>(PG6800_PATH, 5); }

    // PPMd8 Decompression Tests - Level 5
    #[test] fn decompression_ppmd8_apache2_5() { test_decompression::<PPMd8>(APACHE2_PATH, 5); }
    #[test] fn decompression_ppmd8_gpl3_5() { test_decompression::<PPMd8>(GPL3_PATH, 5); }
    #[test] fn decompression_ppmd8_issue3_5() { test_decompression::<PPMd8>(ISSUE3_PATH, 5); }
    #[test] fn decompression_ppmd8_pg100_5() { test_decompression::<PPMd8>(PG100_PATH, 5); }
    #[test] fn decompression_ppmd8_pg6800_5() { test_decompression::<PPMd8>(PG6800_PATH, 5); }

    // PPMd7 Compression Tests - Level 6
    #[test] fn compression_ppmd7_apache2_6() { test_compression::<PPMd7>(APACHE2_PATH, 6); }
    #[test] fn compression_ppmd7_gpl3_6() { test_compression::<PPMd7>(GPL3_PATH, 6); }
    #[test] fn compression_ppmd7_issue3_6() { test_compression::<PPMd7>(ISSUE3_PATH, 6); }
    #[test] fn compression_ppmd7_pg100_6() { test_compression::<PPMd7>(PG100_PATH, 6); }
    #[test] fn compression_ppmd7_pg6800_6() { test_compression::<PPMd7>(PG6800_PATH, 6); }

    // PPMd7 Decompression Tests - Level 6
    #[test] fn decompression_ppmd7_apache2_6() { test_decompression::<PPMd7>(APACHE2_PATH, 6); }
    #[test] fn decompression_ppmd7_gpl3_6() { test_decompression::<PPMd7>(GPL3_PATH, 6); }
    #[test] fn decompression_ppmd7_issue3_6() { test_decompression::<PPMd7>(ISSUE3_PATH, 6); }
    #[test] fn decompression_ppmd7_pg100_6() { test_decompression::<PPMd7>(PG100_PATH, 6); }
    #[test] fn decompression_ppmd7_pg6800_6() { test_decompression::<PPMd7>(PG6800_PATH, 6); }

    // PPMd8 Compression Tests - Level 6
    #[test] fn compression_ppmd8_apache2_6() { test_compression::<PPMd8>(APACHE2_PATH, 6); }
    #[test] fn compression_ppmd8_gpl3_6() { test_compression::<PPMd8>(GPL3_PATH, 6); }
    #[test] fn compression_ppmd8_issue3_6() { test_compression::<PPMd8>(ISSUE3_PATH, 6); }
    #[test] fn compression_ppmd8_pg100_6() { test_compression::<PPMd8>(PG100_PATH, 6); }
    #[test] fn compression_ppmd8_pg6800_6() { test_compression::<PPMd8>(PG6800_PATH, 6); }

    // PPMd8 Decompression Tests - Level 6
    #[test] fn decompression_ppmd8_apache2_6() { test_decompression::<PPMd8>(APACHE2_PATH, 6); }
    #[test] fn decompression_ppmd8_gpl3_6() { test_decompression::<PPMd8>(GPL3_PATH, 6); }
    #[test] fn decompression_ppmd8_issue3_6() { test_decompression::<PPMd8>(ISSUE3_PATH, 6); }
    #[test] fn decompression_ppmd8_pg100_6() { test_decompression::<PPMd8>(PG100_PATH, 6); }
    #[test] fn decompression_ppmd8_pg6800_6() { test_decompression::<PPMd8>(PG6800_PATH, 6); }

    // PPMd7 Compression Tests - Level 7
    #[test] fn compression_ppmd7_apache2_7() { test_compression::<PPMd7>(APACHE2_PATH, 7); }
    #[test] fn compression_ppmd7_gpl3_7() { test_compression::<PPMd7>(GPL3_PATH, 7); }
    #[test] fn compression_ppmd7_issue3_7() { test_compression::<PPMd7>(ISSUE3_PATH, 7); }
    #[test] fn compression_ppmd7_pg100_7() { test_compression::<PPMd7>(PG100_PATH, 7); }
    #[test] fn compression_ppmd7_pg6800_7() { test_compression::<PPMd7>(PG6800_PATH, 7); }

    // PPMd7 Decompression Tests - Level 7
    #[test] fn decompression_ppmd7_apache2_7() { test_decompression::<PPMd7>(APACHE2_PATH, 7); }
    #[test] fn decompression_ppmd7_gpl3_7() { test_decompression::<PPMd7>(GPL3_PATH, 7); }
    #[test] fn decompression_ppmd7_issue3_7() { test_decompression::<PPMd7>(ISSUE3_PATH, 7); }
    #[test] fn decompression_ppmd7_pg100_7() { test_decompression::<PPMd7>(PG100_PATH, 7); }
    #[test] fn decompression_ppmd7_pg6800_7() { test_decompression::<PPMd7>(PG6800_PATH, 7); }

    // PPMd8 Compression Tests - Level 7
    #[test] fn compression_ppmd8_apache2_7() { test_compression::<PPMd8>(APACHE2_PATH, 7); }
    #[test] fn compression_ppmd8_gpl3_7() { test_compression::<PPMd8>(GPL3_PATH, 7); }
    #[test] fn compression_ppmd8_issue3_7() { test_compression::<PPMd8>(ISSUE3_PATH, 7); }
    #[test] fn compression_ppmd8_pg100_7() { test_compression::<PPMd8>(PG100_PATH, 7); }
    #[test] fn compression_ppmd8_pg6800_7() { test_compression::<PPMd8>(PG6800_PATH, 7); }

    // PPMd8 Decompression Tests - Level 7
    #[test] fn decompression_ppmd8_apache2_7() { test_decompression::<PPMd8>(APACHE2_PATH, 7); }
    #[test] fn decompression_ppmd8_gpl3_7() { test_decompression::<PPMd8>(GPL3_PATH, 7); }
    #[test] fn decompression_ppmd8_issue3_7() { test_decompression::<PPMd8>(ISSUE3_PATH, 7); }
    #[test] fn decompression_ppmd8_pg100_7() { test_decompression::<PPMd8>(PG100_PATH, 7); }
    #[test] fn decompression_ppmd8_pg6800_7() { test_decompression::<PPMd8>(PG6800_PATH, 7); }

    // PPMd7 Compression Tests - Level 8
    #[test] fn compression_ppmd7_apache2_8() { test_compression::<PPMd7>(APACHE2_PATH, 8); }
    #[test] fn compression_ppmd7_gpl3_8() { test_compression::<PPMd7>(GPL3_PATH, 8); }
    #[test] fn compression_ppmd7_issue3_8() { test_compression::<PPMd7>(ISSUE3_PATH, 8); }
    #[test] fn compression_ppmd7_pg100_8() { test_compression::<PPMd7>(PG100_PATH, 8); }
    #[test] fn compression_ppmd7_pg6800_8() { test_compression::<PPMd7>(PG6800_PATH, 8); }

    // PPMd7 Decompression Tests - Level 8
    #[test] fn decompression_ppmd7_apache2_8() { test_decompression::<PPMd7>(APACHE2_PATH, 8); }
    #[test] fn decompression_ppmd7_gpl3_8() { test_decompression::<PPMd7>(GPL3_PATH, 8); }
    #[test] fn decompression_ppmd7_issue3_8() { test_decompression::<PPMd7>(ISSUE3_PATH, 8); }
    #[test] fn decompression_ppmd7_pg100_8() { test_decompression::<PPMd7>(PG100_PATH, 8); }
    #[test] fn decompression_ppmd7_pg6800_8() { test_decompression::<PPMd7>(PG6800_PATH, 8); }

    // PPMd8 Compression Tests - Level 8
    #[test] fn compression_ppmd8_apache2_8() { test_compression::<PPMd8>(APACHE2_PATH, 8); }
    #[test] fn compression_ppmd8_gpl3_8() { test_compression::<PPMd8>(GPL3_PATH, 8); }
    #[test] fn compression_ppmd8_issue3_8() { test_compression::<PPMd8>(ISSUE3_PATH, 8); }
    #[test] fn compression_ppmd8_pg100_8() { test_compression::<PPMd8>(PG100_PATH, 8); }
    #[test] fn compression_ppmd8_pg6800_8() { test_compression::<PPMd8>(PG6800_PATH, 8); }

    // PPMd8 Decompression Tests - Level 8
    #[test] fn decompression_ppmd8_apache2_8() { test_decompression::<PPMd8>(APACHE2_PATH, 8); }
    #[test] fn decompression_ppmd8_gpl3_8() { test_decompression::<PPMd8>(GPL3_PATH, 8); }
    #[test] fn decompression_ppmd8_issue3_8() { test_decompression::<PPMd8>(ISSUE3_PATH, 8); }
    #[test] fn decompression_ppmd8_pg100_8() { test_decompression::<PPMd8>(PG100_PATH, 8); }
    #[test] fn decompression_ppmd8_pg6800_8() { test_decompression::<PPMd8>(PG6800_PATH, 8); }

    // PPMd7 Compression Tests - Level 9
    #[test] fn compression_ppmd7_apache2_9() { test_compression::<PPMd7>(APACHE2_PATH, 9); }
    #[test] fn compression_ppmd7_gpl3_9() { test_compression::<PPMd7>(GPL3_PATH, 9); }
    #[test] fn compression_ppmd7_issue3_9() { test_compression::<PPMd7>(ISSUE3_PATH, 9); }
    #[test] fn compression_ppmd7_pg100_9() { test_compression::<PPMd7>(PG100_PATH, 9); }
    #[test] fn compression_ppmd7_pg6800_9() { test_compression::<PPMd7>(PG6800_PATH, 9); }

    // PPMd7 Decompression Tests - Level 9
    #[test] fn decompression_ppmd7_apache2_9() { test_decompression::<PPMd7>(APACHE2_PATH, 9); }
    #[test] fn decompression_ppmd7_gpl3_9() { test_decompression::<PPMd7>(GPL3_PATH, 9); }
    #[test] fn decompression_ppmd7_issue3_9() { test_decompression::<PPMd7>(ISSUE3_PATH, 9); }
    #[test] fn decompression_ppmd7_pg100_9() { test_decompression::<PPMd7>(PG100_PATH, 9); }
    #[test] fn decompression_ppmd7_pg6800_9() { test_decompression::<PPMd7>(PG6800_PATH, 9); }

    // PPMd8 Compression Tests - Level 9
    #[test] fn compression_ppmd8_apache2_9() { test_compression::<PPMd8>(APACHE2_PATH, 9); }
    #[test] fn compression_ppmd8_gpl3_9() { test_compression::<PPMd8>(GPL3_PATH, 9); }
    #[test] fn compression_ppmd8_issue3_9() { test_compression::<PPMd8>(ISSUE3_PATH, 9); }
    #[test] fn compression_ppmd8_pg100_9() { test_compression::<PPMd8>(PG100_PATH, 9); }
    #[test] fn compression_ppmd8_pg6800_9() { test_compression::<PPMd8>(PG6800_PATH, 9); }

    // PPMd8 Decompression Tests - Level 9
    #[test] fn decompression_ppmd8_apache2_9() { test_decompression::<PPMd8>(APACHE2_PATH, 9); }
    #[test] fn decompression_ppmd8_gpl3_9() { test_decompression::<PPMd8>(GPL3_PATH, 9); }
    #[test] fn decompression_ppmd8_issue3_9() { test_decompression::<PPMd8>(ISSUE3_PATH, 9); }
    #[test] fn decompression_ppmd8_pg100_9() { test_decompression::<PPMd8>(PG100_PATH, 9); }
    #[test] fn decompression_ppmd8_pg6800_9() { test_decompression::<PPMd8>(PG6800_PATH, 9); }
}
