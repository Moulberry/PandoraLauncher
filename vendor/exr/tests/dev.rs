//! Contains some "test" functions that were be used for developing.

extern crate exr;
extern crate smallvec;

use std::{ffi::OsStr, io::Cursor, path::PathBuf};

use exr::{image::validate_results::ValidateResult, meta::header::Header, prelude::*};
use rayon::{iter::ParallelIterator, prelude::IntoParallelIterator};

fn exr_files() -> impl Iterator<Item = PathBuf> {
    walkdir::WalkDir::new("tests/images/valid")
        .into_iter()
        .map(std::result::Result::unwrap)
        .filter(|entry| entry.path().extension() == Some(OsStr::new("exr")))
        .map(walkdir::DirEntry::into_path)
}

#[test]
#[ignore]
fn print_meta_of_all_files() {
    let files: Vec<PathBuf> = exr_files().collect();

    files.into_par_iter().for_each(|path| {
        let meta = MetaData::read_from_file(&path, false);
        println!("{:?}: \t\t\t {:?}", path.file_name().unwrap(), meta.unwrap());
    });
}

#[test]
#[ignore]
fn search_previews_of_all_files() {
    let files: Vec<PathBuf> = exr_files().collect();

    files.into_par_iter().for_each(|path| {
        let meta = MetaData::read_from_file(&path, false).unwrap();
        let has_preview = meta.headers.iter().any(|header: &Header| {
            header.own_attributes.preview.is_some()
                || header.own_attributes.other.values().any(|value| match value {
                    AttributeValue::Preview(_) => true,
                    _ => false,
                })
        });

        if has_preview {
            println!("Found preview attribute in {:?}", path.file_name().unwrap());
        }
    });
}

#[ignore]
#[test]
pub fn convert_to_dwa() {
    let path = "tests/images/valid/custom/compression_methods/f32/pxr24.exr";

    let mut image = read()
        .no_deep_data()
        .all_resolution_levels()
        .all_channels()
        .all_layers()
        .all_attributes()
        .non_parallel()
        .from_file(path)
        .unwrap();

    for layer in &mut image.layer_data {
        layer.encoding.compression = Compression::DWAA(Some(45.0));
    }

    image.write().to_file("debug_dwaa.exr").unwrap();
}

// use this command for big endian testing:
// cross test --target mips-unknown-linux-gnu --verbose --test dev
// test_roundtrip -- --ignored
#[test]
#[ignore]
pub fn test_roundtrip() {
    // works
    // let path = "tests/images/fuzzed/b44_overly_restrictive_assert.exr";
    let path = "tests/images/valid/custom/compression_methods/f32/pxr24.exr";

    // worksn't
    // let path = "tests/images/valid/openexr/Chromaticities/Rec709_YC.exr"; //
    // subsampling let path =
    // "tests/images/valid/openexr/LuminanceChroma/Flowers.exr"; // subsampling

    // let path = "tests/images/valid/openexr/IlmfmlmflmTest/test_native1.exr";
    // let path = "tests/images/valid/openexr/IlmfmlmflmTest/test_native2.exr"; //
    // contains NaN

    // deep data?
    // let path = "tests/images/valid/openexr/v2/Stereo/Balls.exr";
    // let path = "tests/images/valid/openexr/v2/Stereo/Ground.exr";

    println!("{:?}", exr::meta::MetaData::read_from_file(path, true));

    let read_image = read()
        .no_deep_data()
        .all_resolution_levels()
        .all_channels()
        .all_layers()
        .all_attributes()
        .non_parallel();

    let image = read_image.clone().from_file(path).unwrap();

    let mut tmp_bytes = Vec::new();
    image.write().to_buffered(Cursor::new(&mut tmp_bytes)).unwrap();
    image.write().to_file("debug_pxr24.exr").unwrap();

    let image2 = read_image.from_buffered(Cursor::new(tmp_bytes)).unwrap();

    image.assert_equals_result(&image2);
}
