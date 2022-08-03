use std::{
    fmt::Display,
    hash::Hash,
    net::{Ipv4Addr, Ipv6Addr},
};

pub trait IpVersion:
    Copy + Sized + Hash + Eq + Unpin + Send + Sync + Display + 'static + private::Sealed
{
}

impl IpVersion for Ipv4Addr {}
impl IpVersion for Ipv6Addr {}

pub(crate) mod private {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    pub trait Sealed {
        const IS_V4: bool;

        fn to_socket_addr(self) -> SocketAddr;
    }

    impl Sealed for Ipv4Addr {
        const IS_V4: bool = true;

        fn to_socket_addr(self) -> SocketAddr {
            SocketAddr::new(IpAddr::V4(self), 0)
        }
    }
    impl Sealed for Ipv6Addr {
        const IS_V4: bool = false;

        fn to_socket_addr(self) -> SocketAddr {
            SocketAddr::new(IpAddr::V6(self), 0)
        }
    }
}
