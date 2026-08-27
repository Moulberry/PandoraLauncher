#[macro_use]
extern crate bencher;

extern crate exr;
use std::io::Cursor;

use bencher::Bencher;
use exr::prelude::*;

fn write_parallel_zip1_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::ZIP1);
}

fn write_parallel_dwaa_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::DWAA(Some(45.0)));
}

fn write_parallel_piz_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::PIZ);
}

fn write_parallel_pxr24_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::PXR24);
}

fn write_parallel_zip16_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::ZIP16);
}

fn write_uncompressed_to_buffered(bench: &mut Bencher) {
    bench_write_full_image_parallel(bench, Compression::Uncompressed);
}

fn bench_write_full_image_parallel(bench: &mut Bencher, compression: Compression) {
    let path = "tests/images/valid/custom/crowskull/crow_uncompressed.exr";
    let mut image = read_all_flat_layers_from_file(path).unwrap();
    for layer in &mut image.layer_data {
        layer.encoding.compression = compression;
    }

    bench.iter(|| {
        let mut result = Vec::with_capacity(2048 * 4);
        image.write().to_buffered(Cursor::new(&mut result)).unwrap();
        bencher::black_box(result);
    })
}

benchmark_group!(
    write,
    write_parallel_dwaa_to_buffered,
    write_parallel_piz_to_buffered,
    write_parallel_zip1_to_buffered,
    write_parallel_zip16_to_buffered,
    write_parallel_pxr24_to_buffered,
    write_uncompressed_to_buffered
);

benchmark_main!(write);
