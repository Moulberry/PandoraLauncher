//! Unix domain sockets for Windows

#[cfg(windows)]
mod stdnet;

#[cfg(windows)]
pub use crate::stdnet::{
    from_path, AcceptAddrs, AcceptAddrsBuf, SocketAddr, UnixListener, UnixListenerExt, UnixStream,
    UnixStreamExt,
};
