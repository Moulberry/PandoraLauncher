use value_bag::ValueBag;

use std::hint::black_box;

fn criterion_benchmark(c: &mut criterion::Criterion) {
    c.bench_function("capture u8 from", |b| {
        b.iter(|| black_box(ValueBag::from(1u8)))
    });
    c.bench_function("capture u8 debug", |b| {
        b.iter(|| black_box(ValueBag::capture_debug(&1u8)))
    });
    c.bench_function("capture string debug", |b| {
        b.iter(|| black_box(ValueBag::capture_debug(&"a string")))
    });
    // Currently at the bottom of the linear list of types in `cast::primitive`
    c.bench_function("capture bool debug", |b| {
        b.iter(|| black_box(ValueBag::capture_debug(&true)))
    });
    c.bench_function("capture custom debug", |b| {
        b.iter(|| {
            #[derive(Debug)]
            struct A;

            black_box(ValueBag::capture_debug(&A))
        })
    });

    c.bench_function("fill custom debug", |b| {
        b.iter(|| {
            black_box(ValueBag::from_fill(&|slot: value_bag::fill::Slot| {
                #[derive(Debug)]
                struct A;

                slot.fill_debug(&A)
            }))
        })
    });

    c.bench_function("capture u8 debug to u64", |b| {
        let v = ValueBag::from(1u8);

        b.iter(|| black_box(v.to_u64()))
    });

    c.bench_function("fill u8 debug to u64", |b| {
        let v = ValueBag::from_fill(&|slot: value_bag::fill::Slot| slot.fill_any(1u8));

        b.iter(|| black_box(v.to_u64()))
    });

    c.bench_function("capture u8 debug to borrowed str", |b| {
        let v = ValueBag::capture_debug(&1u8);

        b.iter(|| black_box(v.to_borrowed_str()))
    });

    c.bench_function("capture str debug to borrowed str", |b| {
        let v = ValueBag::capture_debug(&"a string");

        b.iter(|| black_box(v.to_borrowed_str()))
    });

    c.bench_function("capture str debug to u64", |b| {
        let v = ValueBag::capture_debug(&"a string");

        b.iter(|| black_box(v.to_u64()))
    });

    c.bench_function("capture custom debug to borrowed str", |b| {
        #[derive(Debug)]
        struct A;

        let v = ValueBag::capture_debug(&A);

        b.iter(|| black_box(v.to_borrowed_str()))
    });

    #[cfg(feature = "sval2")]
    {
        c.bench_function("capture u8 sval to u64", |b| {
            let v = ValueBag::from_sval2(&1u8);

            b.iter(|| black_box(v.to_u64()))
        });

        c.bench_function("fill u8 sval to u64", |b| {
            let v = ValueBag::from_fill(&|slot: value_bag::fill::Slot| slot.fill_sval2(&1u8));

            b.iter(|| black_box(v.to_u64()))
        });
    }
}

criterion::criterion_group!(benches, criterion_benchmark);
criterion::criterion_main!(benches);
