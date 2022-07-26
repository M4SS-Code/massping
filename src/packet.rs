use std::{borrow::Cow, marker::PhantomData, mem};

use pnet_packet::{
    icmp::{IcmpPacket, IcmpTypes},
    icmpv6::{Icmpv6Packet, Icmpv6Types},
    ipv4::Ipv4Packet,
    ipv6::Ipv6Packet,
    util, Packet as _,
};

use crate::IpVersion;

pub struct EchoRequestPacket<V: IpVersion> {
    buf: Vec<u8>,
    _version: PhantomData<V>,
}

pub struct EchoReplyPacket<'a, V: IpVersion> {
    buf: Cow<'a, [u8]>,
    _version: PhantomData<V>,
}

impl<V: IpVersion> EchoRequestPacket<V> {
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
    pub(crate) fn from_reply(buf: Cow<'a, [u8]>) -> Option<Self> {
        if V::IS_V4 && let Some(ip_packet) = Ipv4Packet::new(&buf) && let Some(icmp_packet) = IcmpPacket::new(ip_packet.payload()) && icmp_packet.get_icmp_type() == IcmpTypes::EchoReply {
           // SAFETY: we just checked that the packet is valid
            unsafe {Some(Self::from_reply_unchecked(buf))}
        } else if let Some(ip_packet) = Ipv6Packet::new(&buf) && let Some(icmp_packet) = Icmpv6Packet::new(ip_packet.payload()) && icmp_packet.get_icmpv6_type() == Icmpv6Types::EchoReply {
            // SAFETY: we just checked that the packet is valid
            unsafe {Some(Self::from_reply_unchecked(buf))}
        } else {
            None
        }
    }

    pub(crate) unsafe fn from_reply_unchecked(buf: Cow<'a, [u8]>) -> Self {
        Self {
            buf,
            _version: PhantomData,
        }
    }

    pub fn source(&self) -> V {
        if V::IS_V4 {
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv4Packet::new(&self.buf).unwrap_unchecked() };
            let source = packet.get_source();

            // SAFETY: we are transmuting to itself
            unsafe { mem::transmute_copy(&source) }
        } else {
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv6Packet::new(&self.buf).unwrap_unchecked() };
            let source = packet.get_source();

            // SAFETY: we are transmuting to itself
            unsafe { mem::transmute_copy(&source) }
        }
    }

    pub fn identifier(&self) -> u16 {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv4Packet::new(&self.buf).unwrap_unchecked() };
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(packet.payload()).unwrap_unchecked() };
            packet.get_identifier()
        } else {
            use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv6Packet::new(&self.buf).unwrap_unchecked() };
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(packet.payload()).unwrap_unchecked() };
            packet.get_identifier()
        }
    }

    pub fn sequence_number(&self) -> u16 {
        if V::IS_V4 {
            use pnet_packet::icmp::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv4Packet::new(&self.buf).unwrap_unchecked() };
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(packet.payload()).unwrap_unchecked() };
            packet.get_sequence_number()
        } else {
            use pnet_packet::icmpv6::echo_reply::EchoReplyPacket;

            // SAFETY: the check has already been done by the builder
            let packet = unsafe { Ipv6Packet::new(&self.buf).unwrap_unchecked() };
            // SAFETY: the check has already been done by the builder
            let packet = unsafe { EchoReplyPacket::new(packet.payload()).unwrap_unchecked() };
            packet.get_sequence_number()
        }
    }

    pub fn payload(&self) -> &[u8] {
        // TODO: Fix
        &self.buf[self.buf.len() - 64..]
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }
}
