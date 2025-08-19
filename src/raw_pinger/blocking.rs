use std::{
    borrow::Cow, io, marker::PhantomData, mem::MaybeUninit, net::SocketAddr, time::Duration,
};

use crate::{
    IpVersion,
    packet::{EchoReplyPacket, EchoRequestPacket},
    socket::BaseSocket,
};

/// Synchronous pinger
pub struct RawBlockingPinger<V: IpVersion> {
    socket: BaseSocket,
    _version: PhantomData<V>,
}

impl<V: IpVersion> RawBlockingPinger<V> {
    pub fn new() -> io::Result<Self> {
        let socket = BaseSocket::new_icmp::<V>(true, Some(Duration::from_secs(5)))?;

        Ok(Self {
            socket,
            _version: PhantomData,
        })
    }

    /// Send a ICMP ECHO request packet
    pub fn send_to(&self, addr: V, packet: &EchoRequestPacket<V>) -> io::Result<()> {
        let addr = SocketAddr::new(addr.into(), 0);
        self.socket.send_to(packet.as_bytes(), addr).map(|_sent| ())
    }

    /// Receive an ICMP ECHO reply packet
    pub fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<Option<EchoReplyPacket<'_, V>>> {
        let (received, source) = match self.socket.recv(buf) {
            Ok((received, source)) => (received, source),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(err) => return Err(err),
        };
        let source = V::from_ip_addr(source.ip()).unwrap();
        let packet = EchoReplyPacket::from_reply(source, Cow::Borrowed(received));
        Ok(packet)
    }
}
