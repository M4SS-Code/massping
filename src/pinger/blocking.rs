use std::{borrow::Cow, io, marker::PhantomData, mem::MaybeUninit};

use crate::{socket::BaseSocket, EchoReplyPacket, EchoRequestPacket, IpVersion};

pub struct RawBlockingPinger<V: IpVersion> {
    socket: BaseSocket,
    _version: PhantomData<V>,
}

impl<V: IpVersion> RawBlockingPinger<V> {
    pub fn new() -> io::Result<Self> {
        let socket = BaseSocket::new_icmp::<V>(true)?;

        Ok(Self {
            socket,
            _version: PhantomData,
        })
    }

    pub fn send_to(&self, addr: V, packet: &EchoRequestPacket<V>) -> io::Result<()> {
        let addr = addr.to_socket_addr();
        self.socket.send_to(packet.as_bytes(), addr).map(|_sent| ())
    }

    pub fn recv(&self, buf: &mut [MaybeUninit<u8>]) -> io::Result<EchoReplyPacket<'_, V>> {
        loop {
            let received = self.socket.recv(buf)?;

            if let Some(packet) = EchoReplyPacket::from_reply(Cow::Borrowed(received)) {
                return Ok(packet);
            }
        }
    }
}
