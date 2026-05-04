use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use kqueue::Watcher;
use std::{fs::File, path::PathBuf};
use tempfile::{tempdir, TempDir};

const NUMFILES: usize = 3000;
const STEPSIZ: usize = 500;

fn create_files() -> (TempDir, Vec<PathBuf>) {
    let tmp = tempdir().unwrap();
    let mut fils = Vec::new();

    for i in 0..NUMFILES {
        let fil = tmp.path().join(format!("temp{i}"));
        fils.push(fil.clone());
        File::create(fil).unwrap();
    }

    (tmp, fils)
}

fn bench_add(c: &mut Criterion) {
    let (tmp, fils) = create_files();

    let mut group = c.benchmark_group("benches_add");
    for nofiles in (0..=NUMFILES)
        .step_by(STEPSIZ)
        .map(|i| if i == 0 { 1 } else { i })
    {
        group.throughput(criterion::Throughput::Elements(nofiles as u64));
        group.bench_with_input(BenchmarkId::new("add lots", nofiles), &fils, |b, fils| {
            b.iter(|| {
                for fil in fils.iter().take(nofiles) {
                    let mut w = Watcher::new().unwrap();
                    w.add_filename(
                        fil,
                        kqueue::EventFilter::EVFILT_VNODE,
                        kqueue::FilterFlag::empty(),
                    )
                    .unwrap();
                    w.watch().unwrap();
                }
            })
        });
    }

    group.finish();
    drop(tmp);
}

fn bench_del(c: &mut Criterion) {
    let (tmp, fils) = create_files();

    let mut group = c.benchmark_group("benches_del");
    for nofiles in (0..=NUMFILES)
        .step_by(STEPSIZ)
        .map(|i| if i == 0 { 1 } else { i })
    {
        group.throughput(criterion::Throughput::Elements(nofiles as u64));
        group.bench_with_input(BenchmarkId::new("del lots", nofiles), &fils, |b, fils| {
            b.iter_batched(
                || {
                    let mut w = Watcher::new().unwrap();
                    for fil in fils.iter().take(nofiles) {
                        w.add_filename(
                            fil,
                            kqueue::EventFilter::EVFILT_VNODE,
                            kqueue::FilterFlag::empty(),
                        )
                        .unwrap();
                    }
                    w.watch().unwrap();
                    w
                },
                |mut w| {
                    for fil in fils.iter().take(nofiles).rev() {
                        w.remove_filename(fil, kqueue::EventFilter::EVFILT_VNODE)
                            .unwrap();
                    }
                },
                criterion::BatchSize::PerIteration,
            )
        });
    }

    group.finish();
    drop(tmp);
}

criterion_group!(lots, bench_add, bench_del);
criterion_main!(lots);
