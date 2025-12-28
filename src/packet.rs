//! ICMP packets implementation used by [`raw_pinger`].
//!
//! [`raw_pinger`]: crate::raw_pinger

use std::{borrow::Cow, marker::PhantomData};

use pnet_packet::{
    Packet as _,
    icmp::{IcmpPacket, IcmpTypes},
    icmpv6::{Icmpv6Packet, Icmpv6Types},
    util,
};

use crate::IpVersion;

/// An ICMP echo request packet
pub struct EchoRequestPacket<V: IpVersion> {
    buf: Vec<u8>,
    _version: PhantomData<V>,
}

/// An ICMP echo reply packet
pub struct EchoReplyPacket<'a, V: IpVersion> {
    source: V,
    buf: Cow<'a, [u8]>,
    _version: PhantomData<V>,
}

impl<V: IpVersion> EchoRequestPacket<V> {
    /// Build a new ICMP echo request packet
    pub fn new(identifier: u16, sequence_number: u16, payload: &[u8]) -> Self {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_request::MutableEchoRequestPacket;

            let mut buf = vec![0; 8 + payload.len()];

            let mut packet = MutableEchoRequestPacket::new(&mut buf).unwrap();
            packet.set_icmp_type(IcmpTypes::EchoRequest);
            packet.set_identifier(identifier);
            packet.set_sequence_number(sequence_number);
            packet.set_payload(payload);
            packet.set_checksum(util::checksum(packet.packet(), 1));

            let packet_len = packet.packet().len();
            debug_assert_eq!(buf.len(), packet_len);

            Self::from_buf(buf)
        } else {
            use pnet_packet::icmpv6::echo_request::MutableEchoRequestPacket;

            let mut buf = vec![0; 8 + payload.len()];

            let mut packet = MutableEchoRequestPacket::new(&mut buf).unwrap();
            packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
            packet.set_identifier(identifier);
            packet.set_sequence_number(sequence_number);
            packet.set_payload(payload);
            packet.set_checksum(util::checksum(packet.packet(), 1));

            let packet_len = packet.packet().len();
            debug_assert_eq!(buf.len(), packet_len);

            Self::from_buf(buf)
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    fn from_buf(buf: Vec<u8>) -> Self {
        Self {
            buf,
            _version: PhantomData,
        }
    }
}

impl<'a, V: IpVersion> EchoReplyPacket<'a, V> {
    /// Parse an ICMP echo reply packet
    pub(crate) fn from_reply(source: V, buf: Cow<'a, [u8]>) -> Option<Self> {
        if V::IS_V4 {
            if let Some(icmp_packet) = IcmpPacket::new(&buf) {
                if icmp_packet.get_icmp_type() == IcmpTypes::EchoReply {
                    // SAFETY: we just checked that the packet is valid
                    return Some(unsafe { Self::from_reply_unchecked(source, buf) });
                }
            }
        } else if let Some(icmp_packet) = Icmpv6Packet::new(&buf) {
            if icmp_packet.get_icmpv6_type() == Icmpv6Types::EchoReply {
                // SAFETY: we just checked that the packet is valid
                return Some(unsafe { Self::from_reply_unchecked(source, buf) });
            }
        }

        None
    }

    pub(crate) unsafe fn from_reply_unchecked(source: V, buf: Cow<'a, [u8]>) -> Self {
        Self {
            source,
            buf,
            _version: PhantomData,
        }
    }

    /// Get the source IP address
    pub fn source(&self) -> V {
        self.source
    }

    /// Get the ICMP packet identifier
    pub fn identifier(&self) -> u16 {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.get_identifier()
        } else {
            use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.get_identifier()
        }
    }

    /// Get the ICMP packet sequence number
    pub fn sequence_number(&self) -> u16 {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.get_sequence_number()
        } else {
            use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.get_sequence_number()
        }
    }

    /// Get the ICMP packet payload
    pub fn payload(&self) -> &[u8] {
        let payload_len = if V::IS_V4 {
            use pnet_packet::icmp::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.payload().len()
        } else {
            use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(&self.buf).unwrap_unchecked() };
            packet.payload().len()
        };

        // TODO: Fix
        &self.buf[self.buf.len() - payload_len..]
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }
}
