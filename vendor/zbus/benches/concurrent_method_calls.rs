use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use zbus::Message;

fn concurrent_method_calls(c: &mut Criterion) {
    let mut group = c.benchmark_group("method-call");
    group.sample_size(10);
    group.throughput(criterion::Throughput::Elements(
        CONCURRENT_METHOD_CALLS as u64,
    ));

    let (_server, client) = zbus::block_on(create_benchmark_connection_pair());
    group.bench_function("1000-concurrent-p2p", |b| {
        b.iter(|| zbus::block_on(call_ping_concurrently(&client)));
    });
}

async fn create_benchmark_connection_pair() -> (zbus::Connection, zbus::Connection) {
    let (server_socket, client_socket) = zbus::connection::socket::Channel::pair();
    let guid = zbus::Guid::generate();

    let server = zbus::connection::Builder::authenticated_socket(server_socket, guid.clone())
        .unwrap()
        .p2p()
        .serve_at(BENCHMARK_PATH, BenchmarkInterface)
        .unwrap()
        .build();
    let client = zbus::connection::Builder::authenticated_socket(client_socket, guid)
        .unwrap()
        .p2p()
        .method_timeout(std::time::Duration::from_secs(30))
        .build();

    futures_util::try_join!(server, client).unwrap()
}

async fn call_ping_concurrently(client: &zbus::Connection) {
    let replies = futures_util::future::try_join_all(
        (0..CONCURRENT_METHOD_CALLS).map(|value| call_ping(client, value as u32)),
    )
    .await
    .unwrap();
    black_box(replies);
}

async fn call_ping(client: &zbus::Connection, value: u32) -> zbus::Result<Message> {
    client
        .call_method(
            None::<()>,
            BENCHMARK_PATH,
            Some(BENCHMARK_INTERFACE),
            "Ping",
            &value,
        )
        .await
}

struct BenchmarkInterface;

#[zbus::interface(name = "org.zbus.Benchmark")]
impl BenchmarkInterface {
    async fn ping(&self, value: u32) -> u32 {
        value
    }
}

const CONCURRENT_METHOD_CALLS: usize = 1000;
const BENCHMARK_PATH: &str = "/org/zbus/Benchmark";
const BENCHMARK_INTERFACE: &str = "org.zbus.Benchmark";

criterion_group!(benches, concurrent_method_calls);
criterion_main!(benches);
