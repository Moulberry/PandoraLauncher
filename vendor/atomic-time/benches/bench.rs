use std::{
  sync::{atomic::Ordering, Arc},
  time::Duration,
};

use arc_swap::ArcSwap;
use atomic_time::AtomicDuration;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_atomic_duration_read(c: &mut Criterion) {
  let duration = AtomicDuration::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("atomic_duration_read", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        duration.load(Ordering::SeqCst);
      })
    },
  );
}

fn bench_atomic_duration_write(c: &mut Criterion) {
  let duration = AtomicDuration::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("atomic_duration_write", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        duration.store(black_box(Duration::from_nanos(10000)), Ordering::SeqCst);
      })
    },
  );
}

fn bench_arc_swap_duration_read(c: &mut Criterion) {
  let duration = ArcSwap::new(Arc::new(Duration::from_nanos(5000)));
  c.bench_with_input(
    BenchmarkId::new("arc_swap_read", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        duration.load();
      })
    },
  );
}

fn bench_arc_swap_duration_read_full(c: &mut Criterion) {
  let duration = ArcSwap::new(Arc::new(Duration::from_nanos(5000)));
  c.bench_with_input(
    BenchmarkId::new("arc_swap_read_full", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        duration.load_full();
      })
    },
  );
}

fn bench_arc_swap_duration_write(c: &mut Criterion) {
  let duration = ArcSwap::new(Arc::new(Duration::from_nanos(5000)));
  c.bench_with_input(
    BenchmarkId::new("arc_swap_write", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        duration.store(Arc::new(black_box(Duration::from_nanos(10000))));
      })
    },
  );
}

fn bench_parking_rwlock_duration_read(c: &mut Criterion) {
  let duration = parking_lot::RwLock::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("parking_rwlock_read", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        let mu = duration.read();
        let _ = *mu;
      })
    },
  );
}

fn bench_parking_rwlock_duration_write(c: &mut Criterion) {
  let duration = parking_lot::RwLock::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("parking_rwlock_write", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        let mut mu = duration.write();
        *mu = Duration::from_nanos(10000);
      })
    },
  );
}

fn bench_std_rwlock_duration_read(c: &mut Criterion) {
  let duration = std::sync::RwLock::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("std_rwlock_read", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        let mu = duration.read().unwrap();
        let _ = *mu;
      })
    },
  );
}

fn bench_std_rwlock_duration_write(c: &mut Criterion) {
  let duration = std::sync::RwLock::new(Duration::from_nanos(5000));
  c.bench_with_input(
    BenchmarkId::new("std_rwlock_write", ""),
    &duration,
    |b, duration| {
      b.iter(|| {
        let mut mu = duration.write().unwrap();
        *mu = Duration::from_nanos(10000);
      })
    },
  );
}

criterion_group!(
  benches,
  bench_atomic_duration_read,
  bench_atomic_duration_write,
  bench_arc_swap_duration_read,
  bench_arc_swap_duration_read_full,
  bench_arc_swap_duration_write,
  bench_parking_rwlock_duration_read,
  bench_parking_rwlock_duration_write,
  bench_std_rwlock_duration_read,
  bench_std_rwlock_duration_write
);
criterion_main!(benches);
