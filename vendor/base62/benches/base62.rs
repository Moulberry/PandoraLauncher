use base62::{
    decode, decode_alternative, encode, encode_alternative, encode_alternative_buf,
    encode_alternative_bytes, encode_buf, encode_bytes,
};

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rand::distr::StandardUniform;
use rand::{rng, Rng};

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // Fixed input benchmark
    group.bench_function("standard_fixed", |b| {
        b.iter(|| decode(black_box("7n42DGM5Tflk9n8mt7Fhc7")))
    });

    // Random input benchmark using iter_batched for setup
    group.bench_function("standard_random", |b| {
        b.iter_batched(
            || {
                // Setup (runs outside measured time)
                let random_num: u128 = rng().sample(StandardUniform);
                encode(random_num)
            },
            decode, // Measured function
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_fixed", |b| {
        b.iter(|| decode_alternative(black_box("7N42dgm5tFLK9N8MT7fHC7")))
    });

    group.bench_function("alternative_random", |b| {
        b.iter_batched(
            || {
                // Setup (runs outside measured time)
                let random_num: u128 = rng().sample(StandardUniform);
                encode_alternative(random_num)
            },
            decode_alternative,
            BatchSize::SmallInput,
        )
    });
    group.finish();

    let mut group = c.benchmark_group("encode");

    group.bench_function("standard_new_fixed", |b| {
        b.iter(|| encode(black_box(u128::MAX)))
    });

    group.bench_function("standard_new_random", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u128| encode(black_box(num)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("standard_new_random_u64", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u64| encode(black_box(num)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("standard_bytes_fixed", |b| {
        let mut buf = [0; 22];
        b.iter(|| encode_bytes(black_box(u128::MAX), black_box(&mut buf)))
    });

    group.bench_function("standard_bytes_random", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u128| {
                let mut buf = [0; 22];
                encode_bytes(black_box(num), black_box(&mut buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("standard_bytes_random_u64", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u64| {
                let mut buf = [0; 22];
                encode_bytes(black_box(num), black_box(&mut buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("standard_buf_fixed", |b| {
        b.iter(|| encode_buf(black_box(u128::MAX), black_box(&mut String::new())))
    });

    group.bench_function("standard_buf_random", |b| {
        b.iter_batched_ref(
            || {
                let num: u128 = rng().sample(StandardUniform);
                (num, String::with_capacity(22))
            },
            |(num, buf)| {
                buf.clear();
                encode_buf(black_box(*num), black_box(buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("standard_buf_random_u64", |b| {
        b.iter_batched_ref(
            || {
                let num: u64 = rng().sample(StandardUniform);
                (num, String::with_capacity(11))
            },
            |(num, buf)| {
                buf.clear();
                encode_buf(black_box(*num), black_box(buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_new_fixed", |b| {
        b.iter(|| encode_alternative(black_box(u128::MAX)))
    });

    group.bench_function("alternative_new_random", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u128| encode_alternative(black_box(num)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_new_random_u64", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u64| encode_alternative(black_box(num)),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_bytes_fixed", |b| {
        let mut buf = [0; 22];
        b.iter(|| encode_alternative_bytes(black_box(u128::MAX), black_box(&mut buf)))
    });

    group.bench_function("alternative_bytes_random", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u128| {
                let mut buf = [0; 22];
                encode_alternative_bytes(black_box(num), black_box(&mut buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_bytes_random_u64", |b| {
        b.iter_batched(
            || rng().sample(StandardUniform),
            |num: u64| {
                let mut buf = [0; 11];
                encode_alternative_bytes(black_box(num), black_box(&mut buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_buf_fixed", |b| {
        b.iter(|| encode_alternative_buf(black_box(u128::MAX), black_box(&mut String::new())))
    });

    group.bench_function("alternative_buf_random", |b| {
        b.iter_batched_ref(
            || {
                let num: u128 = rng().sample(StandardUniform);
                (num, String::with_capacity(22))
            },
            |(num, buf)| {
                buf.clear();
                encode_alternative_buf(black_box(*num), black_box(buf))
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("alternative_buf_random_u64", |b| {
        b.iter_batched_ref(
            || {
                let num: u64 = rng().sample(StandardUniform);
                (num, String::with_capacity(11))
            },
            |(num, buf)| {
                buf.clear();
                encode_alternative_buf(black_box(*num), black_box(buf))
            },
            BatchSize::SmallInput,
        )
    });

    #[cfg(feature = "std")]
    {
        group.bench_function("standard_io_fixed", |b| {
            let mut buf = Vec::new();
            b.iter(|| {
                buf.clear();
                base62::encode_io(black_box(u128::MAX), black_box(&mut buf))
            })
        });

        group.bench_function("standard_io_random", |b| {
            b.iter_batched_ref(
                || {
                    let num: u128 = rng().sample(StandardUniform);
                    (num, Vec::with_capacity(22))
                },
                |(num, buf)| {
                    buf.clear();
                    base62::encode_io(black_box(*num), black_box(buf))
                },
                BatchSize::SmallInput,
            )
        });

        group.bench_function("standard_io_random_u64", |b| {
            b.iter_batched_ref(
                || {
                    let num: u64 = rng().sample(StandardUniform);
                    (num, Vec::with_capacity(11))
                },
                |(num, buf)| {
                    buf.clear();
                    base62::encode_io(black_box(*num), black_box(buf))
                },
                BatchSize::SmallInput,
            )
        });

        group.bench_function("alternative_io_fixed", |b| {
            let mut buf = Vec::new();
            b.iter(|| {
                buf.clear();
                base62::encode_alternative_io(black_box(u128::MAX), black_box(&mut buf))
            })
        });

        group.bench_function("alternative_io_random", |b| {
            b.iter_batched_ref(
                || {
                    let num: u128 = rng().sample(StandardUniform);
                    (num, Vec::with_capacity(22))
                },
                |(num, buf)| {
                    buf.clear();
                    base62::encode_alternative_io(black_box(*num), black_box(buf))
                },
                BatchSize::SmallInput,
            )
        });

        group.bench_function("alternative_io_random_u64", |b| {
            b.iter_batched_ref(
                || {
                    let num: u64 = rng().sample(StandardUniform);
                    (num, Vec::with_capacity(11))
                },
                |(num, buf)| {
                    buf.clear();
                    base62::encode_alternative_io(black_box(*num), black_box(buf))
                },
                BatchSize::SmallInput,
            )
        });

        use std::io::BufWriter;

        group.bench_function("standard_io_bufwriter_random", |b| {
            b.iter_batched_ref(
                || {
                    let num: u128 = rng().sample(StandardUniform);
                    let vec = Vec::with_capacity(22);
                    (num, BufWriter::new(vec))
                },
                |(num, buf_writer)| {
                    *buf_writer = BufWriter::new(Vec::with_capacity(22));
                    base62::encode_io(black_box(*num), black_box(buf_writer))
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
