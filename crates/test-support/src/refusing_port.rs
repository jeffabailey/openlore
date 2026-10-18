//! A local address that refuses connections and keeps its port (U-50).
//!
//! Binding a port, freeing it and then pointing a client at it races: a
//! parallel test can be handed the freed port first, and the client reaches
//! that test's server instead of a refused connect. [`refusing_address`]
//! never frees the port. It is the local end of a loopback connection to a
//! throwaway listener: a connected socket takes no new connections, so a
//! connect to the address is refused before anything is sent, and the port
//! stays in use while the guard lives, so the OS never hands it to another
//! socket that asks for a free port (`127.0.0.1:0`). (A socket that names the
//! port and sets SO_REUSEADDR can still bind it on macOS; tests here only
//! ever ask for a free port.)
//!
//! Two simpler shapes do not work. A listener that accepts and then resets
//! the connection lets the client send its request first, so the client
//! sees a sent request, not a refused connect. A socket that is only bound
//! (never listening) refuses a connect on Linux but leaves it hanging on
//! macOS.

use std::net::{SocketAddr, TcpListener, TcpStream};

/// Holds a refusing port for as long as it lives. Keep it in scope for the
/// whole time a client may connect to [`RefusingPort::address`].
#[derive(Debug)]
pub struct RefusingPort {
    address: SocketAddr,
    _local_end: TcpStream,
    _far_end: TcpStream,
}

impl RefusingPort {
    /// The `127.0.0.1:<port>` a connect is refused at.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// `http://127.0.0.1:<port>`, for a client that takes a base URL.
    pub fn http_url(&self) -> String {
        format!("http://{}", self.address)
    }
}

/// A loopback address nothing listens on, held by the returned guard.
pub fn refusing_address() -> RefusingPort {
    let far_listener = TcpListener::bind("127.0.0.1:0").expect("refusing_address: bind listener");
    let far_address = far_listener
        .local_addr()
        .expect("refusing_address: listener address");
    let local_end = TcpStream::connect(far_address).expect("refusing_address: connect");
    let (far_end, _) = far_listener.accept().expect("refusing_address: accept");
    RefusingPort {
        address: local_end
            .local_addr()
            .expect("refusing_address: local address"),
        _local_end: local_end,
        _far_end: far_end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn a_connect_to_the_address_is_refused() {
        let port = refusing_address();
        let error = TcpStream::connect(port.address()).expect_err("nothing listens");
        assert_eq!(error.kind(), ErrorKind::ConnectionRefused);
    }
}
