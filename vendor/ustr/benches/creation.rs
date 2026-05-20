#[macro_use]
extern crate criterion;
use criterion::black_box;
use criterion::Criterion;
use crossbeam_channel::bounded;
use crossbeam_utils::thread::scope;
use std::sync::Arc;
use string_cache::DefaultAtom;
use string_interner::StringInterner;

use ustr::*;

use parking_lot::Mutex;

fn criterion_benchmark(c: &mut Criterion) {
    let path =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("data")
            .join("raft-large-directories.txt");
    let raft = std::fs::read_to_string(path).unwrap();
    let raft = Arc::new(
        raft.split_whitespace()
            .collect::<Vec<_>>()
            .chunks(3)
            .map(|s| {
                if s.len() == 3 {
                    format!("{}/{}/{}", s[0], s[1], s[2])
                } else {
                    s[0].to_owned()
                }
            })
            .collect::<Vec<_>>(),
    );

    let s = raft.clone();
    c.bench_function("single raft ustr", move |b| {
        b.iter(|| {
            unsafe { ustr::_clear_cache() };
            for s in s.iter().cycle().take(100_000) {
                black_box(ustr(s));
            }
        });
    });

    let s = raft.clone();
    c.bench_function("single raft string-interner", move |b| {
        b.iter(|| {
            let mut interner = StringInterner::default();
            for s in s.iter().cycle().take(100_000) {
                black_box(interner.get_or_intern(s));
            }
        });
    });

    let s = raft.clone();
    c.bench_function("single raft string-cache", move |b| {
        b.iter(|| {
            let mut v = Vec::with_capacity(100_000);
            for s in s.iter().cycle().take(100_000) {
                v.push(DefaultAtom::from(s.as_str()));
            }
            black_box(v);
        });
    });

    let s = raft.clone();
    c.bench_function("single raft String", move |b| {
        b.iter(|| {
            for s in s.iter().cycle().take(100_000) {
                black_box(String::from(s));
            }
        });
    });

    let num = 100_000;

    for num_threads in [1, 2, 4, 6, 8, 12].iter() {
        let num_threads = *num_threads;

        let s = Arc::clone(&raft);
        c.bench_function(
            &format!("raft ustr x {} threads", num_threads),
            move |b| {
                let (tx1, rx1) = bounded(0);
                let (tx2, rx2) = bounded(0);
                let s = Arc::clone(&s);
                scope(|scope| {
                    for tt in 0..num_threads {
                        let t = tt;
                        let rx1 = rx1.clone();
                        let tx2 = tx2.clone();
                        let s = Arc::clone(&s);
                        scope.spawn(move |_| {
                            while rx1.recv().is_ok() {
                                for s in s.iter().cycle().skip(t * 17).take(num)
                                {
                                    black_box(ustr(s));
                                }
                                tx2.send(()).unwrap();
                            }
                        });
                    }

                    b.iter(|| {
                        unsafe { ustr::_clear_cache() };
                        for _ in 0..num_threads {
                            tx1.send(()).unwrap();
                        }

                        for _ in 0..num_threads {
                            rx2.recv().unwrap();
                        }
                    });
                    drop(tx1);
                })
                .unwrap();
            },
        );

        let s = Arc::clone(&raft);
        c.bench_function(
            &format!("raft string-interner x {} threads", num_threads),
            move |b| {
                let (tx1, rx1) = bounded::<
                    Arc<Mutex<StringInterner<string_interner::DefaultSymbol>>>,
                >(0);
                let (tx2, rx2) = bounded(0);
                scope(|scope| {
                    for tt in 0..num_threads {
                        let t = tt;
                        let rx1 = rx1.clone();
                        let tx2 = tx2.clone();
                        let s = Arc::clone(&s);
                        scope.spawn(move |_| {
                            while let Ok(interner) = rx1.recv() {
                                for s in s.iter().cycle().skip(t * 17).take(num)
                                {
                                    let mut int = interner.lock();
                                    black_box(int.get_or_intern(s));
                                }
                                tx2.send(()).unwrap();
                            }
                        });
                    }

                    b.iter(|| {
                        let interner =
                            Arc::new(Mutex::new(StringInterner::default()));
                        for _ in 0..num_threads {
                            tx1.send(interner.clone()).unwrap();
                        }

                        for _ in 0..num_threads {
                            rx2.recv().unwrap();
                        }
                    });
                    drop(tx1);
                })
                .unwrap();
            },
        );

        let s = Arc::clone(&raft);
        c.bench_function(
            &format!("raft string-cache x {} threads", num_threads),
            move |b| {
                let (tx1, rx1) = bounded(0);
                let (tx2, rx2) = bounded(0);
                scope(|scope| {
                    for tt in 0..num_threads {
                        let t = tt;
                        let rx1 = rx1.clone();
                        let tx2 = tx2.clone();
                        let s = Arc::clone(&s);
                        scope.spawn(move |_| {
                            while rx1.recv().is_ok() {
                                let mut v = Vec::with_capacity(num);
                                for s in s.iter().cycle().skip(t * 17).take(num)
                                {
                                    v.push(DefaultAtom::from(s.as_str()));
                                }
                                tx2.send(()).unwrap();
                            }
                        });
                    }

                    b.iter(|| {
                        for _ in 0..num_threads {
                            tx1.send(()).unwrap();
                        }

                        for _ in 0..num_threads {
                            rx2.recv().unwrap();
                        }
                    });
                    drop(tx1);
                })
                .unwrap();
            },
        );

        let s = Arc::clone(&raft);
        c.bench_function(
            &format!("raft String::from x {} threads", num_threads),
            move |b| {
                let (tx1, rx1) = bounded(0);
                let (tx2, rx2) = bounded(0);
                scope(|scope| {
                    for tt in 0..num_threads {
                        let t = tt;
                        let rx1 = rx1.clone();
                        let tx2 = tx2.clone();
                        let s = Arc::clone(&s);
                        scope.spawn(move |_| {
                            while rx1.recv().is_ok() {
                                for s in s.iter().cycle().skip(t * 17).take(num)
                                {
                                    black_box(String::from(s));
                                }
                                tx2.send(()).unwrap();
                            }
                        });
                    }

                    b.iter(|| {
                        for _ in 0..num_threads {
                            tx1.send(()).unwrap();
                        }

                        for _ in 0..num_threads {
                            rx2.recv().unwrap();
                        }
                    });
                    drop(tx1);
                })
                .unwrap();
            },
        );
    }

    let path =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("data")
            .join("raft-large-directories.txt");
    let raft_large = std::fs::read_to_string(path).unwrap();
    let raft_large = Arc::new(
        raft_large
            .split_whitespace()
            .collect::<Vec<_>>()
            .chunks(11)
            .map(|s| {
                // if s.len() == 3 {
                //     format!("{}/{}/{}", s[0], s[1], s[2])
                // } else {
                //     s[0].to_owned()
                // }
                s.join("/")
            })
            .collect::<Vec<_>>(),
    );

    let s = raft_large.clone();
    c.bench_function("raft large x1", move |b| {
        b.iter(|| {
            unsafe { ustr::_clear_cache() };
            for s in s.iter().cycle().take(100_000) {
                black_box(ustr(s));
            }
        });
    });

    let num_threads = 6;
    let s = raft_large.clone();
    c.bench_function("raft large x6", move |b| {
        let (tx1, rx1) = bounded(0);
        let (tx2, rx2) = bounded(0);
        let s = Arc::clone(&s);
        scope(|scope| {
            for tt in 0..num_threads {
                let t = tt;
                let rx1 = rx1.clone();
                let tx2 = tx2.clone();
                let s = Arc::clone(&s);
                scope.spawn(move |_| {
                    while rx1.recv().is_ok() {
                        for s in s.iter().cycle().skip(t * 17).take(num) {
                            black_box(ustr(s));
                        }
                        tx2.send(()).unwrap();
                    }
                });
            }

            b.iter(|| {
                unsafe { ustr::_clear_cache() };
                for _ in 0..num_threads {
                    tx1.send(()).unwrap();
                }

                for _ in 0..num_threads {
                    rx2.recv().unwrap();
                }
            });
            drop(tx1);
        })
        .unwrap();
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(30);
    targets = criterion_benchmark
);
criterion_main!(benches);
