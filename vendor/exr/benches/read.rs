#[macro_use]
extern crate bencher;

extern crate exr;
use std::{fs, io::Cursor};

use bencher::Bencher;
use exr::{image::pixel_vec::PixelVec, prelude::*};

fn uncompressed_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(
        bench,
        "tests/images/valid/custom/crowskull/crow_uncompressed.exr",
    );
}

fn rle_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(bench, "tests/images/valid/custom/crowskull/crow_rle.exr");
}

fn dwa_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(bench, "tests/images/valid/custom/crowskull/crow_dwa.exr");
}

fn piz_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(bench, "tests/images/valid/custom/crowskull/crow_piz.exr");
}

fn zips_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(bench, "tests/images/valid/custom/crowskull/crow_zips.exr");
}

fn zip_parallel(bench: &mut Bencher) {
    bench_read_full_image_parallel(bench, "tests/images/valid/custom/crowskull/crow_zip.exr");
}

fn uncompressed_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(
        bench,
        "tests/images/valid/custom/crowskull/crow_uncompressed.exr",
    );
}

fn rle_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(bench, "tests/images/valid/custom/crowskull/crow_rle.exr");
}

fn dwa_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(bench, "tests/images/valid/custom/crowskull/crow_dwa.exr");
}

fn piz_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(bench, "tests/images/valid/custom/crowskull/crow_piz.exr");
}

fn zips_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(bench, "tests/images/valid/custom/crowskull/crow_zips.exr");
}

fn zip_non_parallel(bench: &mut Bencher) {
    bench_read_full_image_non_parallel(bench, "tests/images/valid/custom/crowskull/crow_zip.exr");
}

fn bench_read_full_image_parallel(bench: &mut Bencher, path: &str) {
    let mut file = fs::read(path).unwrap();

    bench.iter(|| {
        bencher::black_box(&mut file);

        let image = exr::prelude::read()
            .no_deep_data()
            .largest_resolution_level()
            .all_channels()
            .all_layers()
            .all_attributes()
            .from_buffered(Cursor::new(file.as_slice()))
            .unwrap();

        bencher::black_box(image);
    })
}

fn bench_read_full_image_non_parallel(bench: &mut Bencher, path: &str) {
    let mut file = fs::read(path).unwrap();

    bench.iter(|| {
        bencher::black_box(&mut file);

        let image = exr::prelude::read()
            .no_deep_data()
            .largest_resolution_level()
            .all_channels()
            .all_layers()
            .all_attributes()
            .from_buffered(Cursor::new(file.as_slice()))
            .unwrap();

        bencher::black_box(image);
    })
}

benchmark_group!(
    read,
    zip_non_parallel,
    zips_non_parallel,
    piz_non_parallel,
    dwa_non_parallel,
    rle_non_parallel,
    uncompressed_non_parallel,
    zip_parallel,
    zips_parallel,
    piz_parallel,
    dwa_parallel,
    rle_parallel,
    uncompressed_parallel,
);

benchmark_main!(read);
