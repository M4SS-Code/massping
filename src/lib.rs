//! Asynchronous ICMP ping library using Linux RAW sockets and the
//! tokio runtime.
//!
//! As this crate needs to use RAW sockets, it must either be run as root
//! or permission must explicitly be set via
//! `sudo setcap cap_net_raw=+eip path/to/binary`.
//!
//! ## Features
//!
//! * `strong`: implements strong checking for the RTT. Disabling this
//!             feature makes the pinger a little bit faster, but opens
//!             you up to some servers, like those running [pong][ping],
//!             to faking the ping times.
//! * `stream`: implements [`Stream`] for [`MeasureManyStream`].
//!
//! ## MSRV version policy
//!
//! This project has a CI job to prevent accidental bumping of the MSRV.
//! We might bump MSRV version at any time. If you require a lower MSRV
//! please open an issue.
//!
//! [ping]: https://github.com/m-ou-se/pong
//! [`Stream`]: futures_core::Stream

#![deny(
    rust_2018_idioms,
    clippy::doc_markdown,
    rustdoc::broken_intra_doc_links
)]

use std::{
    io,
    marker::PhantomData,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    task::{Context, Poll},
    time::Duration,
};
#[cfg(feature = "stream")]
use std::{pin::Pin, task::ready};

#[cfg(feature = "stream")]
use futures_core::Stream;

pub use self::{
    ip_version::IpVersion,
    pinger::{MeasureManyStream, Pinger, V4Pinger, V6Pinger},
};

#[cfg(not(feature = "strong"))]
mod instant;
mod ip_version;
pub mod packet;
mod pinger;
pub mod raw_pinger;
mod socket;

/// A pinger for both [`Ipv4Addr`] and [`Ipv6Addr`] addresses.
pub struct DualstackPinger {
    v4: V4Pinger,
    v6: V6Pinger,
}

impl DualstackPinger {
    /// Construct a new `DualstackPinger`.
    ///
    /// For maximum efficiency the same instance of `DualstackPinger` should
    /// be used for as long as possible, altough it might also
    /// be beneficial to `Drop` the `DualstackPinger` and recreate it if
    /// you are not going to be sending pings for a long period of time.
    pub fn new() -> io::Result<Self> {
        let v4 = V4Pinger::new()?;
        let v6 = V6Pinger::new()?;
        Ok(Self { v4, v6 })
    }

    /// Ping `addresses`
    ///
    /// Creates [`DualstackMeasureManyStream`] which **lazily** sends ping
    /// requests and [`Stream`]s the responses as they arrive.
    pub fn measure_many<I>(&self, addresses: I) -> DualstackMeasureManyStream<'_, I>
    where
        I: Iterator<Item = IpAddr> + Clone,
    {
        let addresses_v4 = FilterIpAddr {
            iter: addresses.clone(),
            _marker: PhantomData,
        };
        let addresses_v6 = FilterIpAddr {
            iter: addresses,
            _marker: PhantomData,
        };

        DualstackMeasureManyStream {
            v4: self.v4.measure_many(addresses_v4),
            v6: self.v6.measure_many(addresses_v6),
        }
    }
}

/// A [`Stream`] of ping responses.
///
/// No kind of `rtt` timeout is implemented, so an external mechanism
/// like [`tokio::time::timeout`] should be used to prevent the program
/// from hanging indefinitely.
///
/// Leaking this method might crate a slowly forever growing memory leak.
///
/// [`tokio::time::timeout`]: tokio::time::timeout
pub struct DualstackMeasureManyStream<'a, I: Iterator<Item = IpAddr>> {
    v4: MeasureManyStream<'a, Ipv4Addr, FilterIpAddr<I, Ipv4Addr>>,
    v6: MeasureManyStream<'a, Ipv6Addr, FilterIpAddr<I, Ipv6Addr>>,
}

impl<'a, I: Iterator<Item = IpAddr>> DualstackMeasureManyStream<'a, I> {
    pub fn poll_next_unpin(&mut self, cx: &mut Context<'_>) -> Poll<(IpAddr, Duration)> {
        if let Poll::Ready((v4, rtt)) = self.v4.poll_next_unpin(cx) {
            return Poll::Ready((IpAddr::V4(v4), rtt));
        }

        if let Poll::Ready((v6, rtt)) = self.v6.poll_next_unpin(cx) {
            return Poll::Ready((IpAddr::V6(v6), rtt));
        }

        Poll::Pending
    }
}

#[cfg(feature = "stream")]
impl<'a, I: Iterator<Item = IpAddr> + Unpin> Stream for DualstackMeasureManyStream<'a, I> {
    type Item = (IpAddr, Duration);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = ready!(self.as_mut().poll_next_unpin(cx));
        Poll::Ready(Some(result))
    }
}

struct FilterIpAddr<I, V: IpVersion> {
    iter: I,
    _marker: PhantomData<V>,
}

impl<I: Iterator<Item = IpAddr>, V: IpVersion> Iterator for FilterIpAddr<I, V> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.iter.next()?;
            if let Some(addr) = V::from_ip_addr(item) {
                return Some(addr);
            }
        }
    }
}
