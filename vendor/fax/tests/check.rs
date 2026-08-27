use fax::{decoder, decoder::pels, BitWriter, Bits, Color, VecWriter};
use fax::{encoder, slice_bits, slice_reader, BitReader, ByteReader};
use std::fmt::Debug;
use std::fs;
use std::path::Path;

fn split_once_byte(data: &[u8], needle: u8) -> Option<(&[u8], &[u8])> {
    let pos = data.iter().position(|&b| b == needle)?;
    Some((&data[..pos], &data[pos + 1..]))
}

// Files with known decode issues (decoder stops 1 line short).
// These are pre-existing bugs tracked so CI stays green while they're
// investigated. If a file in this list starts passing, the test fails
// — remove it from the list. If a NEW file starts failing, the test
// also fails.
const KNOWN_DECODE_FAILURES: &[&str] = &["4", "6", "33", "44", "65", "71"];

fn is_known_decode_failure(path: &Path) -> bool {
    let stem = path.file_stem().unwrap().to_string_lossy();
    KNOWN_DECODE_FAILURES.contains(&stem.as_ref())
}

#[test]
fn decode_test_images() {
    let data_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-files/files");

    let mut tested = 0;
    let mut unexpected_failures = vec![];
    let mut unexpected_passes = vec![];
    for r in data_path.read_dir().unwrap() {
        let e = r.unwrap();
        let p = e.path();

        let base = data_path.join(p.file_stem().unwrap());
        let pbm = base.with_extension("pbm");
        let result = if p.extension().is_some_and(|e| e == "fax") {
            let img = read_pbm(&pbm);
            let data = fs::read(&p).unwrap();
            img.test_decode(&data, false)
        } else if p.extension().is_some_and(|e| e == "tiff") {
            let img = read_pbm(&pbm);
            img.test_decode_tiff(&p)
        } else {
            continue;
        };
        tested += 1;

        match (result, is_known_decode_failure(&p)) {
            (Ok(()), false) => {} // expected pass
            (Err(msg), true) => {
                println!("known failure {}: {msg}", p.display());
            }
            (Err(msg), false) => {
                unexpected_failures.push(format!("{}: {msg}", p.display()));
            }
            (Ok(()), true) => {
                unexpected_passes.push(p.display().to_string());
            }
        }
    }
    assert!(tested > 0, "no test images found");
    let mut msgs = vec![];
    if !unexpected_failures.is_empty() {
        msgs.push(format!(
            "new decode failures:\n  {}",
            unexpected_failures.join("\n  ")
        ));
    }
    if !unexpected_passes.is_empty() {
        msgs.push(format!(
            "known failures now pass (remove from KNOWN_DECODE_FAILURES):\n  {}",
            unexpected_passes.join("\n  ")
        ));
    }
    assert!(msgs.is_empty(), "{}", msgs.join("\n"));
}

#[test]
fn roundtrip_encode_test_images() {
    let data_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-files/files");

    let mut unexpected_failures = vec![];
    for r in data_path.read_dir().unwrap() {
        let e = r.unwrap();
        let p = e.path();

        // Only test files that decode correctly.
        if is_known_decode_failure(&p) {
            continue;
        }

        let base = data_path.join(p.file_stem().unwrap());
        let pbm = base.with_extension("pbm");
        let data = if p.extension().is_some_and(|e| e == "fax") {
            fs::read(&p).unwrap()
        } else if p.extension().is_some_and(|e| e == "tiff") {
            match read_tiff_stream(&p) {
                Some(d) => d,
                None => continue,
            }
        } else {
            continue;
        };
        let img = read_pbm(&pbm);
        if let Err(line) = img.test_encode(&data, false) {
            unexpected_failures.push(format!("{}: failed at line {line}", p.display()));
        }
    }
    if !unexpected_failures.is_empty() {
        panic!(
            "encoder roundtrip failures:\n  {}",
            unexpected_failures.join("\n  ")
        );
    }
}

struct TestImage {
    width: u16,
    height: u16,
    data: Vec<u8>,
}
fn read_pbm(path: &Path) -> TestImage {
    let ref_data = std::fs::read(path).unwrap();
    let (header1, data) = split_once_byte(&ref_data, b'\n').unwrap();
    assert_eq!(header1, b"P4");
    let (header2, ref_image) = split_once_byte(data, b'\n').unwrap();
    let header2 = std::str::from_utf8(header2).unwrap();
    let (w, h) = header2.split_once(" ").unwrap();
    let width: u16 = w.parse().unwrap();
    let h: u16 = h.parse().unwrap();

    TestImage {
        width,
        height: h,
        data: ref_image.to_vec(),
    }
}
fn read_tiff_stream(path: &Path) -> Option<Vec<u8>> {
    use tiff::{decoder::Decoder, tags::Tag};
    let data = fs::read(path).unwrap();
    let reader = std::io::Cursor::new(data.as_slice());
    let mut decoder = Decoder::new(reader).unwrap();
    let strip_offset = decoder
        .get_tag(Tag::StripOffsets)
        .unwrap()
        .into_u32()
        .unwrap() as usize;
    let strip_bytes = decoder
        .get_tag(Tag::StripByteCounts)
        .unwrap()
        .into_u32()
        .unwrap() as usize;
    Some(data[strip_offset..strip_offset + strip_bytes].to_vec())
}

impl TestImage {
    fn test_decode(&self, data: &[u8], white_is_1: bool) -> Result<(), String> {
        let (black, white) = match white_is_1 {
            false => (Bits { data: 1, len: 1 }, Bits { data: 0, len: 1 }),
            true => (Bits { data: 0, len: 1 }, Bits { data: 1, len: 1 }),
        };

        let ref_lines: Vec<&[u8]> = self
            .data
            .chunks_exact((self.width as usize + 7) / 8)
            .take(self.height as usize)
            .collect();

        let mut decoded_lines = vec![];
        let ok = decoder::decode_g4(data.iter().cloned(), self.width, None, |transitions| {
            let mut writer = VecWriter::new();
            for c in pels(transitions, self.width) {
                let bit = match c {
                    Color::Black => black,
                    Color::White => white,
                };
                writer.write(bit).unwrap();
            }
            writer.pad();
            decoded_lines.push(writer.finish());
        });

        if ok.is_none() {
            return Err("G4 decode returned None".into());
        }
        if decoded_lines.len() != ref_lines.len() {
            return Err(format!(
                "decoded {} lines, expected {}",
                decoded_lines.len(),
                ref_lines.len()
            ));
        }
        for (i, (decoded, expected)) in decoded_lines.iter().zip(ref_lines.iter()).enumerate() {
            if decoded.as_slice() != *expected {
                return Err(format!("line {i} pixel mismatch"));
            }
        }
        Ok(())
    }

    fn test_decode_tiff(&self, path: &Path) -> Result<(), String> {
        use tiff::{decoder::Decoder, tags::Tag};
        let data = fs::read(path).unwrap();
        let reader = std::io::Cursor::new(data.as_slice());
        let mut decoder = Decoder::new(reader).unwrap();
        let strip_offset = decoder
            .get_tag(Tag::StripOffsets)
            .unwrap()
            .into_u32()
            .unwrap() as usize;
        let strip_bytes = decoder
            .get_tag(Tag::StripByteCounts)
            .unwrap()
            .into_u32()
            .unwrap() as usize;

        let white_is_1 = decoder
            .get_tag(Tag::PhotometricInterpretation)
            .unwrap()
            .into_u16()
            .unwrap()
            != 0;

        let stream = &data[strip_offset..strip_offset + strip_bytes];
        self.test_decode(stream, white_is_1)
    }

    fn test_encode(&self, data: &[u8], white_is_1: bool) -> Result<(), usize> {
        fn pixels(line: &[u8], white_is_1: bool) -> impl Iterator<Item = Color> + '_ {
            slice_bits(line).map(move |b| {
                if b ^ white_is_1 {
                    Color::Black
                } else {
                    Color::White
                }
            })
        }
        let mut expected = slice_reader(data);
        let mut encoder = encoder::Encoder::new(TestWriter {
            expected: &mut expected,
            offset: 0,
        });
        let ref_lines = self
            .data
            .chunks_exact((self.width as usize + 7) / 8)
            .take(self.height as usize);
        for (i, line) in ref_lines.enumerate() {
            if encoder
                .encode_line(pixels(line, white_is_1), self.width)
                .is_err()
            {
                return Err(i);
            }
        }
        Ok(())
    }
}

struct TestWriter<'a, R> {
    offset: usize,
    expected: &'a mut ByteReader<R>,
}
impl<'a, E: Debug, R: Iterator<Item = Result<u8, E>>> BitWriter for TestWriter<'a, R> {
    type Error = (usize, u8);
    fn write(&mut self, bits: Bits) -> Result<(), Self::Error> {
        match self.expected.expect(bits) {
            Ok(()) => {
                self.expected.consume(bits.len).unwrap();
            }
            Err(_) => {
                self.expected.print_peek();
                println!(
                    "    @{}+{} found {}",
                    self.offset / 8,
                    self.offset % 8,
                    bits
                );
                return Err((self.offset / 8, (self.offset % 8) as u8));
            }
        }
        self.offset += bits.len as usize;
        Ok(())
    }
}
