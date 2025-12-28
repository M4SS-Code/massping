use std::{
    io,
    mem::MaybeUninit,
    net::SocketAddr,
    os::unix::io::{AsRawFd, RawFd},
    slice,
};

use socket2::{Domain, Protocol, SockAddr, Type};

use crate::IpVersion;

pub(crate) struct BaseSocket {
    socket: socket2::Socket,
}

impl BaseSocket {
    pub(crate) fn new_icmp<V: IpVersion>() -> io::Result<Self> {
        let socket = if V::IS_V4 {
            Self::new_icmpv4()
        } else {
            Self::new_icmpv6()
        }?;

        Ok(Self { socket })
    }

    fn new_icmpv4() -> io::Result<socket2::Socket> {
        socket2::Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::ICMPV4))
    }

    fn new_icmpv6() -> io::Result<socket2::Socket> {
        socket2::Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::ICMPV6))
    }

    pub(crate) fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<(&'_ [u8], SocketAddr)> {
        self.socket.recv_from(buf).map(|(filled, source)| {
            (
                unsafe { slice::from_raw_parts(buf.as_ptr().cast::<u8>(), filled) },
                source.as_socket().expect("SockAddr is an IP socket"),
            )
        })
    }

    pub(crate) fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        let addr = SockAddr::from(addr);

        self.socket.send_to(buf, &addr)
    }
}

impl AsRawFd for BaseSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}
