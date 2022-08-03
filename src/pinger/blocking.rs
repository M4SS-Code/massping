use std::{
    borrow::Cow,
    io,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    net::IpAddr,
    time::Duration,
};

use crate::{socket::BaseSocket, EchoReplyPacket, EchoRequestPacket, IpVersion};

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

    pub fn send_to(&self, addr: V, packet: &EchoRequestPacket<V>) -> io::Result<()> {
        let addr = addr.to_socket_addr();
        self.socket.send_to(packet.as_bytes(), addr).map(|_sent| ())
    }

    pub fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<Option<EchoReplyPacket<'_, V>>> {
        let (received, source) = match self.socket.recv(buf) {
            Ok((received, source)) => (received, source),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(err) => return Err(err),
        };
        let source = match (source.ip(), V::IS_V4) {
            (IpAddr::V4(v4), true) => unsafe { mem::transmute_copy(&v4) },
            (IpAddr::V6(v6), false) => unsafe { mem::transmute_copy(&v6) },
            _ => unreachable!(),
        };

        let packet = EchoReplyPacket::from_reply(source, Cow::Borrowed(received));
        Ok(packet)
    }
}
