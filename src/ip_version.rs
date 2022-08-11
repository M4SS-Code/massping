use std::{
    hash::Hash,
    net::{Ipv4Addr, Ipv6Addr},
};

/// Either an [`Ipv4Addr`] or an [`Ipv6Addr`].
pub trait IpVersion: Copy + Hash + Eq + Unpin + Send + Sync + 'static + private::Sealed {}

impl IpVersion for Ipv4Addr {}
impl IpVersion for Ipv6Addr {}

pub(crate) mod private {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    pub trait Sealed: Sized {
        const IS_V4: bool;

        fn to_socket_addr(self) -> SocketAddr;

        fn from_ip_addr(addr: IpAddr) -> Option<Self>;
    }

    impl Sealed for Ipv4Addr {
        const IS_V4: bool = true;

        fn to_socket_addr(self) -> SocketAddr {
            SocketAddr::new(IpAddr::V4(self), 0)
        }

        fn from_ip_addr(addr: IpAddr) -> Option<Self> {
            match addr {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            }
        }
    }
    impl Sealed for Ipv6Addr {
        const IS_V4: bool = false;

        fn to_socket_addr(self) -> SocketAddr {
            SocketAddr::new(IpAddr::V6(self), 0)
        }

        fn from_ip_addr(addr: IpAddr) -> Option<Self> {
            match addr {
                IpAddr::V4(_) => None,
                IpAddr::V6(v6) => Some(v6),
            }
        }
    }
}
