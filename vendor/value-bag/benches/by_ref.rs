use value_bag::ValueBag;

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("by_ref u8", |b| {
        let v = ValueBag::from(1u8);

        b.iter(|| black_box(v.by_ref()))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
