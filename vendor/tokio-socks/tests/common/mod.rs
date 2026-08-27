#![allow(dead_code)]
#[cfg(feature = "tokio")]
use std::net::{IpAddr, Ipv6Addr, SocketAddr};

#[cfg(feature = "tokio")]
use futures_util::stream;
#[cfg(feature = "tokio")]
use tokio_socks::{Result, ToProxyAddrs};

#[cfg(feature = "futures-io")]
pub mod futures_utils;
#[cfg(feature = "tokio")]
pub mod tokio_utils;

#[cfg(feature = "tokio")]
pub use tokio_utils::*;

pub const UNIX_PROXY_ADDR: &str = "/tmp/proxy.s";
pub const PROXY_ADDR: &str = "127.0.0.1:41080";
pub const UNIX_SOCKS4_PROXY_ADDR: &str = "/tmp/socks4_proxy.s";
pub const SOCKS4_PROXY_ADDR: &str = "127.0.0.1:41081";
pub const ECHO_SERVER_ADDR: &str = "localhost:10007";
pub const MSG: &[u8] = b"hello";

#[cfg(feature = "tokio")]
pub struct ProxyAddrsWithUnreachableFirst(pub &'static str);

#[cfg(feature = "tokio")]
impl ToProxyAddrs for ProxyAddrsWithUnreachableFirst {
    type Output = stream::Iter<std::vec::IntoIter<Result<SocketAddr>>>;

    fn to_proxy_addrs(&self) -> Self::Output {
        let reachable = self.0.parse::<SocketAddr>().unwrap();
        let unreachable = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)),
            reachable.port(),
        );

        stream::iter(vec![Ok(unreachable), Ok(reachable)])
    }
}
