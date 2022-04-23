//! A simplified version of [fastping-rs](https://github.com/bparli/fastping-rs)
//! without some of its [issues](https://github.com/bparli/fastping-rs/issues/25).
//!
//! Depends on the tokio 1 runtime.
//!
//! Tested on: Linux
//!
//! As with the original version, this one also requires to create raw sockets,
//! so the permission must either be explicitly set
//! (`sudo setcap cap_net_raw=eip /path/to/binary` for example) or be run as root.

use std::{
    collections::BTreeMap,
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::mpsc,
    time::{Duration, Instant},
};

use pnet_packet::{ip::IpNextHeaderProtocols, util, Packet};
use pnet_transport::{
    icmp_packet_iter, icmpv6_packet_iter, transport_channel, TransportChannelType,
    TransportProtocol,
};
use tokio::task;

/// Ping `addrs` with a max RTT of `rtt` and a packet size of `size`
///
/// Requires to be run inside the `tokio` 1 context.
pub async fn ping(
    addrs: impl Iterator<Item = IpAddr>,
    rtt: Duration,
    size: u16,
) -> io::Result<BTreeMap<IpAddr, Option<Duration>>> {
    let mut received = BTreeMap::new();

    let mut v4s = Vec::new();
    let mut v6s = Vec::new();

    for addr in addrs {
        match addr {
            IpAddr::V4(v4) => {
                v4s.push(v4);
            }
            IpAddr::V6(v6) => {
                v6s.push(v6);
            }
        }
    }

    let received4 = (!v4s.is_empty())
        .then(|| task::spawn_blocking(move || ping_v4(v4s.into_iter(), rtt, size)));

    let received6 = (!v6s.is_empty())
        .then(|| task::spawn_blocking(move || ping_v6(v6s.into_iter(), rtt, size)));

    if let Some(received4) = received4 {
        let received4 = received4.await??;
        received.extend(
            received4
                .into_iter()
                .map(|(v4, took)| (IpAddr::V4(v4), took)),
        );
    }

    if let Some(received6) = received6 {
        let received6 = received6.await??;
        received.extend(
            received6
                .into_iter()
                .map(|(v6, took)| (IpAddr::V6(v6), took)),
        );
    }

    Ok(received)
}

/// Ping `addrs` with a max RTT of `rtt` and a packet size of `size`
///
/// NOTE: this function blocks the current thread for up to `rtt`.
///
/// Requires to be run inside the `tokio` 1 context.
pub fn ping_v4(
    addrs: impl Iterator<Item = Ipv4Addr>,
    rtt: Duration,
    size: u16,
) -> io::Result<BTreeMap<Ipv4Addr, Option<Duration>>> {
    use pnet_packet::icmp::{
        echo_reply::EchoReplyPacket, echo_request::MutableEchoRequestPacket, IcmpTypes,
    };

    let protocol =
        TransportChannelType::Layer4(TransportProtocol::Ipv4(IpNextHeaderProtocols::Icmp));
    let (mut raw_tx, mut raw_rx) = transport_channel(4096, protocol)?;
    let (tx, rx) = mpsc::channel::<(Ipv4Addr, Instant)>();

    task::spawn_blocking(move || {
        let mut receiver = icmp_packet_iter(&mut raw_rx);

        let start_time = Instant::now();
        while let Some(remaining) = rtt.checked_sub(start_time.elapsed()) {
            match receiver.next_with_timeout(remaining) {
                Ok(Some((packet, addr))) => {
                    if let Some(reply) = EchoReplyPacket::new(packet.packet()) {
                        if reply.get_icmp_type() == IcmpTypes::EchoReply {
                            if let IpAddr::V4(v4) = addr {
                                let now = Instant::now();

                                if tx.send((v4, now)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });

    let mut sent_count = 0;
    let mut sent = BTreeMap::default();

    let mut packet_vec = vec![0u8; usize::from(size)];
    for addr in addrs {
        getrandom::getrandom(&mut packet_vec).unwrap();

        let mut packet = MutableEchoRequestPacket::new(&mut packet_vec).unwrap();
        packet.set_sequence_number(1);
        packet.set_identifier(1);
        packet.set_icmp_type(IcmpTypes::EchoRequest);
        packet.set_checksum(util::checksum(packet.packet(), 1));

        if raw_tx.send_to(packet, IpAddr::V4(addr)).is_ok() {
            sent_count += 1;
        }

        let now = Instant::now();
        sent.insert(addr, now);
    }

    let mut received_count = 0;
    let mut received = sent
        .keys()
        .map(|&ip| (ip, None))
        .collect::<BTreeMap<_, _>>();
    for (ip, received_at) in rx.into_iter() {
        if let Some(sent_at) = sent.get_mut(&ip) {
            let took = received_at.saturating_duration_since(*sent_at);

            if let Some(space) = received.get_mut(&ip) {
                if space.is_none() {
                    *space = Some(took);
                    received_count += 1;

                    if received_count == sent_count {
                        break;
                    }
                }
            }
        }
    }
    Ok(received)
}

/// Ping `addrs` with a max RTT of `rtt` and a packet size of `size`
///
/// NOTE: this function blocks the current thread for up to `rtt`.
///
/// Requires to be run inside the `tokio` 1 context.
pub fn ping_v6(
    addrs: impl Iterator<Item = Ipv6Addr>,
    rtt: Duration,
    size: u16,
) -> io::Result<BTreeMap<Ipv6Addr, Option<Duration>>> {
    use pnet_packet::icmpv6::{
        echo_reply::EchoReplyPacket, echo_request::MutableEchoRequestPacket, Icmpv6Types,
    };

    let protocol =
        TransportChannelType::Layer4(TransportProtocol::Ipv6(IpNextHeaderProtocols::Icmpv6));
    let (mut raw_tx, mut raw_rx) = transport_channel(4096, protocol)?;
    let (tx, rx) = mpsc::channel::<(Ipv6Addr, Instant)>();

    task::spawn_blocking(move || {
        let mut receiver = icmpv6_packet_iter(&mut raw_rx);

        let start_time = Instant::now();
        while let Some(remaining) = rtt.checked_sub(start_time.elapsed()) {
            match receiver.next_with_timeout(remaining) {
                Ok(Some((packet, addr))) => {
                    if let Some(reply) = EchoReplyPacket::new(packet.packet()) {
                        if reply.get_icmpv6_type() == Icmpv6Types::EchoReply {
                            if let IpAddr::V6(v6) = addr {
                                let now = Instant::now();

                                if tx.send((v6, now)).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });

    let mut sent_count = 0;
    let mut sent = BTreeMap::default();

    let mut packet_vec = vec![0u8; usize::from(size)];
    for addr in addrs {
        getrandom::getrandom(&mut packet_vec).unwrap();

        let mut packet = MutableEchoRequestPacket::new(&mut packet_vec).unwrap();
        packet.set_sequence_number(1);
        packet.set_identifier(1);
        packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
        packet.set_checksum(util::checksum(packet.packet(), 1));

        if raw_tx.send_to(packet, IpAddr::V6(addr)).is_ok() {
            sent_count += 1;
        }

        let now = Instant::now();
        sent.insert(addr, now);
    }

    let mut received_count = 0;
    let mut received = sent
        .keys()
        .map(|&ip| (ip, None))
        .collect::<BTreeMap<_, _>>();
    for (ip, received_at) in rx.into_iter() {
        if let Some(sent_at) = sent.get_mut(&ip) {
            let took = received_at.saturating_duration_since(*sent_at);

            if let Some(space) = received.get_mut(&ip) {
                if space.is_none() {
                    *space = Some(took);
                    received_count += 1;

                    if received_count == sent_count {
                        break;
                    }
                }
            }
        }
    }
    Ok(received)
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[tokio::test]
    async fn ipv4() {
        let localhost = "127.0.0.1".parse().unwrap();
        let one_one_one_one = "1.1.1.1".parse().unwrap();
        let not_answering = "0.0.0.1".parse().unwrap();

        let addrs = [localhost, one_one_one_one, not_answering].into_iter();
        let rtt = Duration::from_secs(5);
        let size = 64;
        let pings = ping_v4(addrs, rtt, size).unwrap();
        assert_eq!(pings.len(), 3);
        assert!(pings.get(&localhost).unwrap().unwrap() < Duration::from_secs(1));
        // GitHub Actions doesn't support ping
        if env::var("CI").is_err() {
            assert!(pings.get(&one_one_one_one).unwrap().unwrap() < rtt);
            assert!(pings.get(&not_answering).unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn ipv6() {
        let localhost = "::1".parse().unwrap();
        let one_one_one_one = "2606:4700:4700::1111".parse().unwrap();

        let addrs = [localhost, one_one_one_one].into_iter();
        let rtt = Duration::from_secs(5);
        let size = 64;
        let pings = ping_v6(addrs, rtt, size).unwrap();
        assert_eq!(pings.len(), 2);
        assert!(pings.get(&localhost).unwrap().unwrap() < Duration::from_secs(1));
        // GitHub Actions doesn't support ping
        if env::var("CI").is_err() {
            assert!(pings.get(&one_one_one_one).unwrap().unwrap() < rtt);
        }
    }

    #[tokio::test]
    async fn both() {
        let localhost_v4 = "127.0.0.1".parse().unwrap();
        let one_one_one_one_v4 = "1.1.1.1".parse().unwrap();
        let not_answering_v4 = "0.0.0.1".parse().unwrap();
        let localhost_v6 = "::1".parse().unwrap();
        let one_one_one_one_v6 = "2606:4700:4700::1111".parse().unwrap();

        let addrs = [
            localhost_v4,
            one_one_one_one_v4,
            not_answering_v4,
            localhost_v6,
            one_one_one_one_v6,
        ]
        .into_iter();
        let rtt = Duration::from_secs(5);
        let size = 64;
        let pings = ping(addrs, rtt, size).await.unwrap();
        assert_eq!(pings.len(), 5);
        assert!(pings.get(&localhost_v4).unwrap().unwrap() < Duration::from_secs(1));
        assert!(pings.get(&not_answering_v4).unwrap().is_none());
        assert!(pings.get(&localhost_v6).unwrap().unwrap() < Duration::from_secs(1));

        // GitHub Actions doesn't support ping
        if env::var("CI").is_err() {
            assert!(pings.get(&one_one_one_one_v4).unwrap().unwrap() < rtt);
            assert!(pings.get(&one_one_one_one_v6).unwrap().unwrap() < rtt);
        }
    }
}
