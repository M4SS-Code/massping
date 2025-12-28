//! Synchronous and asynchronous raw pinger implementation

use std::{
    io,
    marker::PhantomData,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll, ready},
};

use bytes::BytesMut;

use crate::{
    IpVersion,
    packet::{EchoReplyPacket, EchoRequestPacket},
    socket::Socket,
};

pub type RawV4Pinger = RawPinger<Ipv4Addr>;
pub type RawV6Pinger = RawPinger<Ipv6Addr>;

/// Asynchronous pinger
pub struct RawPinger<V: IpVersion> {
    socket: Socket,
    _version: PhantomData<V>,
}

impl<V: IpVersion> RawPinger<V> {
    pub fn new() -> io::Result<Self> {
        let socket = Socket::new_icmp::<V>()?;

        Ok(Self {
            socket,
            _version: PhantomData,
        })
    }

    /// Send a ICMP ECHO request packet
    pub fn send_to<'a>(&'a self, addr: V, packet: &'a EchoRequestPacket<V>) -> SendFuture<'a, V> {
        SendFuture {
            pinger: self,
            addr,
            packet,
        }
    }

    /// Send a ICMP ECHO request packet
    pub fn poll_send_to(
        &self,
        cx: &mut Context<'_>,
        addr: V,
        packet: &EchoRequestPacket<V>,
    ) -> Poll<io::Result<()>> {
        let addr = SocketAddr::new(addr.into(), 0);

        let result = ready!(self.socket.poll_write_to(cx, packet.as_bytes(), addr));
        Poll::Ready(result.map(|_sent| ()))
    }

    /// Receive an ICMP ECHO reply packet
    pub fn recv(&self) -> RecvFuture<'_, V> {
        RecvFuture {
            pinger: self,
            buf: BytesMut::new(),
        }
    }

    /// Receive an ICMP ECHO reply packet
    pub fn poll_recv(
        &self,
        buf: &mut BytesMut,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<EchoReplyPacket<V>>> {
        let (buf, source) = ready!(self.socket.poll_read(buf, cx))?;
        let source = V::from_ip_addr(source.ip()).unwrap();
        match EchoReplyPacket::from_reply(source, buf) {
            Some(packet) => Poll::Ready(Ok(packet)),
            None => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

/// [`Future`] obtained from [`RawPinger::send_to`].
pub struct SendFuture<'a, V: IpVersion> {
    pinger: &'a RawPinger<V>,
    addr: V,
    packet: &'a EchoRequestPacket<V>,
}

impl<V: IpVersion> Future for SendFuture<'_, V> {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.pinger.poll_send_to(cx, self.addr, self.packet)
    }
}

/// [`Future`] obtained from [`RawPinger::recv`].
pub struct RecvFuture<'a, V: IpVersion> {
    pinger: &'a RawPinger<V>,
    buf: BytesMut,
}

impl<V: IpVersion> Future for RecvFuture<'_, V> {
    type Output = io::Result<EchoReplyPacket<V>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let packet = ready!(self.pinger.poll_recv(&mut self.buf, cx))?;
        // SAFETY: `RawPinger` already checked that the packet is valid
        Poll::Ready(Ok(packet))
    }
}
