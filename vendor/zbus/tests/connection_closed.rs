#![cfg(all(unix, feature = "p2p", feature = "async-io"))]

use std::os::unix::net::UnixStream;

use ntest::timeout;
use test_log::test;
use zbus::{Guid, block_on, connection::Builder};

#[test]
#[timeout(15000)]
fn closed_resolves_when_peer_disconnects() {
    block_on(async {
        let (s0, s1) = UnixStream::pair().unwrap();
        let guid = Guid::generate();

        let server_builder = Builder::async_io_unix_stream(s0)
            .server(guid)
            .unwrap()
            .p2p();
        let client_builder = Builder::async_io_unix_stream(s1).p2p();

        let (server, client) = futures_util::join!(server_builder.build(), client_builder.build());
        let server = server.unwrap();
        let client = client.unwrap();

        assert!(!server.is_closed());

        // Drop the client connection, closing the socket.
        drop(client);

        // closed() should resolve now that the peer is gone.
        server.closed().await;
        assert!(server.is_closed());
    });
}

#[test]
#[timeout(15000)]
fn closed_resolves_on_explicit_close() {
    block_on(async {
        let (s0, s1) = UnixStream::pair().unwrap();
        let guid = Guid::generate();

        let server_builder = Builder::async_io_unix_stream(s0)
            .server(guid)
            .unwrap()
            .p2p();
        let client_builder = Builder::async_io_unix_stream(s1).p2p();

        let (server, client) = futures_util::join!(server_builder.build(), client_builder.build());
        let server = server.unwrap();
        let client = client.unwrap();

        assert!(!client.is_closed());

        client.close().await.unwrap();

        // closed() should resolve after an explicit close() call.
        server.closed().await;
        assert!(server.is_closed());
    });
}

#[test]
#[timeout(15000)]
fn closed_resolves_immediately_if_already_closed() {
    block_on(async {
        let (s0, s1) = UnixStream::pair().unwrap();
        let guid = Guid::generate();

        let server_builder = Builder::async_io_unix_stream(s0)
            .server(guid)
            .unwrap()
            .p2p();
        let client_builder = Builder::async_io_unix_stream(s1).p2p();

        let (server, client) = futures_util::join!(server_builder.build(), client_builder.build());
        let server = server.unwrap();
        let client = client.unwrap();

        drop(client);
        server.closed().await;

        // Calling closed() again should return immediately.
        server.closed().await;
        assert!(server.is_closed());
    });
}
