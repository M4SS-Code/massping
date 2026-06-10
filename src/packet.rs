//! ICMP packets implementation used by [`raw_pinger`].
//!
//! [`raw_pinger`]: crate::raw_pinger

use std::marker::PhantomData;

use bytes::{Bytes, BytesMut};

use crate::IpVersion;

// ICMP header layout (8 bytes):
// Offset 0: Type (u8)
// Offset 1: Code (u8)
// Offset 2-3: Checksum (u16 big-endian)
// Offset 4-5: Identifier (u16 big-endian)
// Offset 6-7: Sequence Number (u16 big-endian)
// Offset 8+: Payload
const ICMP_HEADER_LEN: usize = 8;

const ICMPV4_TYPE_ECHO_REQUEST: u8 = 8;
const ICMPV4_TYPE_ECHO_REPLY: u8 = 0;
const ICMPV6_TYPE_ECHO_REQUEST: u8 = 128;
const ICMPV6_TYPE_ECHO_REPLY: u8 = 129;

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
    ///
    /// Note that when the packet is sent through a Linux `SOCK_DGRAM` ICMP
    /// socket ("ping socket"), as done by this crate, the kernel overwrites
    /// `identifier` with the socket's own identifier and recomputes the
    /// checksum. The kernel also delivers only the echo replies whose
    /// identifier matches the socket, so replies don't need to be checked
    /// against the value given here.
    pub fn new(identifier: u16, sequence_number: u16, payload: &[u8]) -> Self {
        let mut buf = BytesMut::zeroed(ICMP_HEADER_LEN + payload.len());

        let echo_type = if V::IS_V4 {
            ICMPV4_TYPE_ECHO_REQUEST
        } else {
            ICMPV6_TYPE_ECHO_REQUEST
        };

        buf[0] = echo_type;
        // buf[1] = 0 (code, already zeroed)
        // buf[2..4] = checksum, computed below
        buf[4..6].copy_from_slice(&identifier.to_be_bytes());
        buf[6..8].copy_from_slice(&sequence_number.to_be_bytes());
        buf[ICMP_HEADER_LEN..].copy_from_slice(payload);

        let checksum = internet_checksum(&buf);
        buf[2..4].copy_from_slice(&checksum.to_be_bytes());

        Self {
            buf: buf.freeze(),
            _version: PhantomData,
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Get the payload of this echo request.
    pub(crate) fn payload(&self) -> Bytes {
        self.buf.slice(ICMP_HEADER_LEN..)
    }
}

impl<V: IpVersion> EchoReplyPacket<V> {
    /// Parse an ICMP echo reply packet
    pub(crate) fn from_reply(source: V, buf: Bytes) -> Option<Self> {
        if buf.len() < ICMP_HEADER_LEN {
            return None;
        }

        let expected_type = if V::IS_V4 {
            ICMPV4_TYPE_ECHO_REPLY
        } else {
            ICMPV6_TYPE_ECHO_REPLY
        };

        if buf[0] != expected_type {
            return None;
        }

        let identifier = u16::from_be_bytes([buf[4], buf[5]]);
        let sequence_number = u16::from_be_bytes([buf[6], buf[7]]);
        let payload = buf.slice(ICMP_HEADER_LEN..);

        Some(Self {
            source,
            identifier,
            sequence_number,
            payload,
        })
    }

    /// Get the source IP address
    pub fn source(&self) -> V {
        self.source
    }

    /// Get the ICMP packet identifier
    ///
    /// On Linux ping sockets this is the kernel-assigned identifier of the
    /// receiving socket, not the value passed to [`EchoRequestPacket::new`].
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

/// Compute the internet checksum (RFC 1071) over the given data.
fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;

    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }

    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !sum as u16
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use bytes::Bytes;

    use super::{EchoReplyPacket, EchoRequestPacket, internet_checksum};

    /// Well-known example from the "Internet checksum" Wikipedia article:
    /// an IPv4 header (checksum field zeroed) whose checksum is 0xB861.
    #[test]
    fn internet_checksum_reference_vector() {
        let ip_header = [
            0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0x00, 0x00, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        assert_eq!(internet_checksum(&ip_header), 0xb861);
    }

    #[test]
    fn internet_checksum_empty() {
        assert_eq!(internet_checksum(&[]), 0xffff);
    }

    /// The end-around carry must be folded back into the sum.
    #[test]
    fn internet_checksum_folds_carry() {
        assert_eq!(internet_checksum(&[0xff; 8]), 0x0000);
    }

    /// The trailing byte of odd-length data is padded with a zero byte,
    /// i.e. it forms the high-order byte of the last 16-bit word.
    #[test]
    fn internet_checksum_odd_length() {
        assert_eq!(internet_checksum(&[0x01, 0x02, 0x03]), !(0x0102 + 0x0300));
    }

    #[test]
    fn echo_request_packet_v4_layout() {
        let packet = EchoRequestPacket::<Ipv4Addr>::new(0x1234, 0x5678, b"test");
        let buf = packet.as_bytes();

        assert_eq!(buf.len(), 12);
        assert_eq!(buf[0], 8, "ICMPv4 echo request type");
        assert_eq!(buf[1], 0, "code");
        assert_eq!(buf[2..4], 0xa779u16.to_be_bytes(), "checksum");
        assert_eq!(buf[4..6], 0x1234u16.to_be_bytes(), "identifier");
        assert_eq!(buf[6..8], 0x5678u16.to_be_bytes(), "sequence number");
        assert_eq!(&buf[8..], b"test");

        // A packet whose checksum field is correct sums to zero.
        assert_eq!(internet_checksum(buf), 0);
        assert_eq!(&packet.payload()[..], b"test");
    }

    #[test]
    fn echo_request_packet_v6_uses_icmpv6_type() {
        // Odd-length payload to also exercise the checksum padding.
        let packet = EchoRequestPacket::<Ipv6Addr>::new(1, 2, b"abc");
        let buf = packet.as_bytes();

        assert_eq!(buf[0], 128, "ICMPv6 echo request type");
        // The kernel recomputes the ICMPv6 checksum (it includes a
        // pseudo-header userspace can't know), but the packet must still be
        // self-consistent under the plain internet checksum.
        assert_eq!(internet_checksum(buf), 0);
    }

    #[test]
    fn from_reply_rejects_truncated_packet() {
        // Too short to be a valid ICMP packet (needs at least 8 bytes)
        let buf = Bytes::from_static(&[0x00, 0x00]);
        let result = EchoReplyPacket::<Ipv4Addr>::from_reply(Ipv4Addr::LOCALHOST, buf);
        assert!(result.is_none(), "Should reject truncated packet");
    }

    #[test]
    fn from_reply_rejects_wrong_icmp_type() {
        // ICMP Echo Request (type 8) instead of Echo Reply (type 0)
        // Format: type(1), code(1), checksum(2), identifier(2), sequence(2)
        let buf = Bytes::from_static(&[0x08, 0x00, 0x00, 0x00, 0x12, 0x34, 0x00, 0x01]);
        let result = EchoReplyPacket::<Ipv4Addr>::from_reply(Ipv4Addr::LOCALHOST, buf);
        assert!(result.is_none(), "Should reject Echo Request (type 8)");
    }

    #[test]
    fn from_reply_rejects_destination_unreachable() {
        // ICMP Destination Unreachable (type 3)
        let buf = Bytes::from_static(&[0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let result = EchoReplyPacket::<Ipv4Addr>::from_reply(Ipv4Addr::LOCALHOST, buf);
        assert!(result.is_none(), "Should reject Destination Unreachable");
    }

    #[test]
    fn from_reply_accepts_valid_echo_reply() {
        // Valid ICMP Echo Reply (type 0)
        // Format: type(1), code(1), checksum(2), identifier(2), sequence(2), payload...
        let buf = Bytes::from_static(&[
            0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x00, 0x01, b't', b'e', b's', b't',
        ]);
        let packet = EchoReplyPacket::<Ipv4Addr>::from_reply(Ipv4Addr::LOCALHOST, buf).unwrap();
        assert_eq!(packet.identifier(), 0x1234);
        assert_eq!(packet.sequence_number(), 0x0001);
        assert_eq!(packet.payload(), b"test");
    }
}
