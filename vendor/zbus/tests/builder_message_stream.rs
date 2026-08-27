//! Tests for [`zbus::connection::Builder::build_message_stream`].
//!
//! Simulates the busd scenario: a bus client pipelines a `Hello` method call as part of its
//! SASL handshake, before the server (bus-impl) has started polling for messages. With plain
//! `build()` + `MessageStream::from`, the Hello can be lost; `build_message_stream` fixes
//! this by setting up the stream before the socket reader task starts.

#![cfg(all(unix, feature = "bus-impl"))]

use std::os::unix::net::UnixStream;

use futures_util::StreamExt;
use ntest::timeout;
use test_log::test;
use zbus::{Connection, Guid, block_on, connection::Builder};

/// Simulates the busd race: a bus client pipelines Hello during SASL auth, before the server
/// has started polling. `build_message_stream` must still deliver it.
#[allow(deprecated)] // exercises the deprecated `unix_stream`, which must keep working
#[test]
#[timeout(15000)]
fn build_message_stream_does_not_drop_pipelined_hello() {
    block_on(async {
        let (s0, s1) = UnixStream::pair().unwrap();
        let guid = Guid::generate();

        // Server: SASL auth, set up stream, receive Hello, reply to it.
        let server = async {
            let mut stream = Builder::unix_stream(s0)
                .server(guid)
                .unwrap()
                .p2p()
                .build_message_stream()
                .await
                .unwrap();

            let hello = stream
                .next()
                .await
                .expect("stream terminated unexpectedly")
                .unwrap();
            assert_eq!(hello.header().member().unwrap().as_str(), "Hello");

            // Reply so the client's build() can complete.
            let conn = Connection::from(stream);
            conn.reply(&hello.header(), &(":1.1",)).await.unwrap();
        };

        // Run both concurrently so the SASL handshake completes cooperatively.
        // The client is a bus connection whose build() pipelines Hello during SASL auth.
        let ((), client) = futures_util::join!(server, Builder::unix_stream(s1).build());
        let _client_conn = client.unwrap();
    });
}
