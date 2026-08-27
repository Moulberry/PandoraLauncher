use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

use zbus_xml::Node;

fn benchmarks(c: &mut Criterion) {
    // The largest of the real-world introspection documents in the test data.
    let xml = include_str!("../tests/data/real_world/systemd1_manager.xml");

    c.bench_function("parse_systemd_manager", |b| {
        b.iter(|| Node::try_from(black_box(xml)).unwrap())
    });

    let node = Node::try_from(xml).unwrap();
    c.bench_function("write_systemd_manager", |b| {
        b.iter(|| {
            let mut writer = Vec::with_capacity(xml.len());
            node.to_writer(black_box(&mut writer)).unwrap();

            writer
        })
    });
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
