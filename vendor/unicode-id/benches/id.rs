extern crate criterion;
extern crate unicode_id;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use unicode_id::UnicodeID;

fn bench_unicode_id(c: &mut Criterion) {
    let unicode_chars = chars(1..0x3000);
    let ascii_chars = chars(1..0x80);

    let mut group = c.benchmark_group("UnicodeID");
    group.throughput(Throughput::Bytes(unicode_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_id_start", "unicode"),
        &unicode_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeID::is_id_start).last()),
    );
    group.throughput(Throughput::Bytes(ascii_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_id_start", "ascii"),
        &ascii_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeID::is_id_start).last()),
    );
    group.throughput(Throughput::Bytes(unicode_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_id_continue", "unicode"),
        &unicode_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeID::is_id_continue).last()),
    );
    group.throughput(Throughput::Bytes(ascii_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_id_continue", "ascii"),
        &ascii_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeID::is_id_continue).last()),
    );
    group.finish();
}

fn chars(range: std::ops::Range<u32>) -> Vec<char> {
    range.filter_map(|i| std::char::from_u32(i)).collect()
}

criterion_group!(benches, bench_unicode_id);
criterion_main!(benches);
