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
//!             to taking the ping times.
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
