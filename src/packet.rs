//! ICMP packets implementation used by [`raw_pinger`].
//!
//! [`raw_pinger`]: crate::raw_pinger

use std::marker::PhantomData;

use bytes::{Bytes, BytesMut};
use pnet_packet::{
    Packet as _,
    icmp::{IcmpPacket, IcmpTypes},
    icmpv6::{Icmpv6Packet, Icmpv6Types},
    util,
};

use crate::IpVersion;

/// An ICMP echo request packet
pub struct EchoRequestPacket<V: IpVersion> {
    buf: Bytes,
    _version: PhantomData<V>,
}

/// An ICMP echo reply packet
pub struct EchoReplyPacket<V: IpVersion> {
    source: V,
    identifier: u16,
    sequence_number: u16,
    payload: Bytes,
}

impl<V: IpVersion> EchoRequestPacket<V> {
    /// Build a new ICMP echo request packet
    pub fn new(identifier: u16, sequence_number: u16, payload: &[u8]) -> Self {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_request::MutableEchoRequestPacket;

            let mut buf = BytesMut::zeroed(8 + payload.len());

            let mut packet = MutableEchoRequestPacket::new(&mut buf).unwrap();
            packet.set_icmp_type(IcmpTypes::EchoRequest);
            packet.set_identifier(identifier);
            packet.set_sequence_number(sequence_number);
            packet.set_payload(payload);
            packet.set_checksum(util::checksum(packet.packet(), 1));

            let packet_len = packet.packet().len();
            debug_assert_eq!(buf.len(), packet_len);

            Self::from_buf(buf.freeze())
        } else {
            use pnet_packet::icmpv6::echo_request::MutableEchoRequestPacket;

            let mut buf = BytesMut::zeroed(8 + payload.len());

            let mut packet = MutableEchoRequestPacket::new(&mut buf).unwrap();
            packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
            packet.set_identifier(identifier);
            packet.set_sequence_number(sequence_number);
            packet.set_payload(payload);
            packet.set_checksum(util::checksum(packet.packet(), 1));

            let packet_len = packet.packet().len();
            debug_assert_eq!(buf.len(), packet_len);

            Self::from_buf(buf.freeze())
        }
    }

    fn from_buf(buf: Bytes) -> Self {
        Self {
            buf,
            _version: PhantomData,
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }
}

impl<V: IpVersion> EchoReplyPacket<V> {
    /// Parse an ICMP echo reply packet
    pub(crate) fn from_reply(source: V, buf: Bytes) -> Option<Self> {
        if V::IS_V4 {
            if let Some(icmp_packet) = IcmpPacket::new(&buf) {
                if icmp_packet.get_icmp_type() == IcmpTypes::EchoReply {
                    use pnet_packet::icmp::echo_reply::EchoReplyPacket;

                    if let Some(echo_reply_packet) = EchoReplyPacket::new(&buf) {
                        return Some(Self {
                            source,
                            identifier: echo_reply_packet.get_identifier(),
                            sequence_number: echo_reply_packet.get_sequence_number(),
                            payload: buf.slice_ref(echo_reply_packet.payload()),
                        });
                    }
                }
            }
        } else if let Some(icmp_packet) = Icmpv6Packet::new(&buf) {
            if icmp_packet.get_icmpv6_type() == Icmpv6Types::EchoReply {
                use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

                if let Some(echo_reply_packet) = EchoReplyPacket::new(&buf) {
                    return Some(Self {
                        source,
                        identifier: echo_reply_packet.get_identifier(),
                        sequence_number: echo_reply_packet.get_sequence_number(),
                        payload: buf.slice_ref(echo_reply_packet.payload()),
                    });
                }
            }
        }

        None
    }

    /// Get the source IP address
    pub fn source(&self) -> V {
        self.source
    }

    /// Get the ICMP packet identifier
    pub fn identifier(&self) -> u16 {
        self.identifier
    }

    /// Get the ICMP packet sequence number
    pub fn sequence_number(&self) -> u16 {
        self.sequence_number
    }

    /// Get the ICMP packet payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}
