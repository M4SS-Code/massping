use std::{
    hash::Hash,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

/// Either an [`Ipv4Addr`] or an [`Ipv6Addr`].
pub trait IpVersion:
    Copy + Hash + Eq + Unpin + Send + Sync + Into<IpAddr> + 'static + private::Sealed
{
}

impl IpVersion for Ipv4Addr {}
impl IpVersion for Ipv6Addr {}

pub(crate) mod private {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    pub trait Sealed: Sized {
        const IS_V4: bool;

        fn from_ip_addr(addr: IpAddr) -> Option<Self>;
    }

    impl Sealed for Ipv4Addr {
        const IS_V4: bool = true;

        fn from_ip_addr(addr: IpAddr) -> Option<Self> {
            match addr {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            }
        }
    }
    impl Sealed for Ipv6Addr {
        const IS_V4: bool = false;

        fn from_ip_addr(addr: IpAddr) -> Option<Self> {
            match addr {
                IpAddr::V4(_) => None,
                IpAddr::V6(v6) => Some(v6),
            }
        }
    }
}
