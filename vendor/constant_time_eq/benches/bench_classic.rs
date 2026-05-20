use constant_time_eq::classic::{constant_time_eq, constant_time_eq_n};
use core::hint::black_box;
use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};

fn bench_array(c: &mut Criterion) {
    let mut group = c.benchmark_group("classic::constant_time_eq_n");

    fn bench_array_n<const N: usize>(group: &mut BenchmarkGroup<WallTime>) {
        let input = (&[1; N], &[2; N]);
        group.throughput(Throughput::Bytes(N as u64));
        group.bench_with_input(BenchmarkId::from_parameter(N), &input, |b, &(x, y)| {
            b.iter(|| constant_time_eq_n(black_box(x), black_box(y)))
        });
    }

    bench_array_n::<8>(&mut group);
    bench_array_n::<16>(&mut group);
    bench_array_n::<20>(&mut group);
    bench_array_n::<32>(&mut group);
    bench_array_n::<64>(&mut group);
    bench_array_n::<96>(&mut group);
    bench_array_n::<128>(&mut group);

    group.finish();
}

fn bench_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("classic::constant_time_eq");

    let input = (&[1; 65536], &[2; 65536]);
    for &size in &[8, 16, 20, 32, 64, 96, 128, 4 * 1024, 16 * 1024, 64 * 1024] {
        let input = (&input.0[..size], &input.1[..size]);
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &input, |b, &(x, y)| {
            b.iter(|| constant_time_eq(black_box(x), black_box(y)))
        });
    }

    group.finish();
}

criterion_group!(benches, bench_array, bench_slice);
criterion_main!(benches);
