use ntest::timeout;
use zbus::{Error, connection};

const UNIX_ADDRESS: &str = "unix:path=/this/path/does/not/exist";
const TCP_ADDRESS: &str = "tcp:host=localhost,port=4142,family=ipv4";
#[cfg(unix)]
const UNIXEXEC_ADDRESS: &str = "unixexec:path=/this/path/does/not/exist";
#[cfg(any(
    all(feature = "vsock", not(feature = "tokio")),
    feature = "tokio-vsock"
))]
const VSOCK_ADDRESS: &str = "vsock:cid=2,port=0";

#[test]
#[timeout(15000)]
fn connection_error() {
    // Addresses issue [#1478](https://github.com/z-galaxy/zbus/issues/1478). The issue mentions
    // that connection error troubleshooting could be simplified by surfacing the connection
    // address to the user. This test ensures connection failures throw the error Error::Connection,
    // and that such error shows the address involved in the attempted connection.
    zbus::block_on(connection_error_async());
}

async fn connection_error_async() {
    #[allow(unused_mut)]
    let mut addresses = vec![UNIX_ADDRESS, TCP_ADDRESS];
    #[cfg(unix)]
    addresses.push(UNIXEXEC_ADDRESS);
    #[cfg(any(
        all(feature = "vsock", not(feature = "tokio")),
        feature = "tokio-vsock"
    ))]
    addresses.push(VSOCK_ADDRESS);

    for addr in addresses {
        let res = connection::Builder::address(addr).unwrap().build().await;

        let Err(Error::Connection(_, error_addr)) = res else {
            panic!("expected a connection error, got {res:?}");
        };

        assert_eq!(error_addr.to_string(), addr);
    }
}
