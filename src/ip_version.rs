use std::{
    fmt::Display,
    hash::Hash,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

pub trait IpVersion: Copy + Sized + Hash + Eq + Unpin + Send + Sync + Display + 'static {
    const IS_V4: bool;

    fn to_socket_addr(self) -> SocketAddr;
}

impl IpVersion for Ipv4Addr {
    const IS_V4: bool = true;

    fn to_socket_addr(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self), 0)
    }
}
impl IpVersion for Ipv6Addr {
    const IS_V4: bool = false;

    fn to_socket_addr(self) -> SocketAddr {
        SocketAddr::new(IpAddr::V6(self), 0)
    }
}
