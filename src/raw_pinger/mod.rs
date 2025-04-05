//! Synchronous and asynchronous raw pinger implementation

use std::{
    borrow::Cow,
    future::Future,
    io,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    pin::Pin,
    task::{ready, Context, Poll},
};

pub use self::blocking::RawBlockingPinger;
use crate::{
    packet::{EchoReplyPacket, EchoRequestPacket},
    socket::Socket,
    IpVersion,
};

pub type RawV4BlockingPinger = RawBlockingPinger<Ipv4Addr>;
pub type RawV6BlockingPinger = RawBlockingPinger<Ipv6Addr>;
pub type RawV4Pinger = RawPinger<Ipv4Addr>;
pub type RawV6Pinger = RawPinger<Ipv6Addr>;

mod blocking;

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
            buf: Vec::with_capacity(1600),
        }
    }

    /// Receive an ICMP ECHO reply packet
    pub fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [MaybeUninit<u8>],
    ) -> Poll<io::Result<EchoReplyPacket<'_, V>>> {
        let (buf, source) = ready!(self.socket.poll_read(cx, buf))?;
        let source = V::from_ip_addr(source.ip()).unwrap();
        match EchoReplyPacket::from_reply(source, Cow::Borrowed(buf)) {
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
    buf: Vec<u8>,
}

impl<V: IpVersion> Future for RecvFuture<'_, V> {
    type Output = io::Result<EchoReplyPacket<'static, V>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();

        let packet = ready!(this.pinger.poll_recv(cx, this.buf.spare_capacity_mut()))?;
        let source = packet.source();
        let filled_buf_len = packet.as_bytes().len();

        // SAFETY: `poll_recv` guarantees that `filled_buf_len` have been filled
        unsafe {
            this.buf.set_len(filled_buf_len);
        }

        let buf = mem::replace(&mut this.buf, Vec::with_capacity(1600));
        // SAFETY: `RawPinger` already checked that the packet is valid
        Poll::Ready(Ok(unsafe {
            EchoReplyPacket::from_reply_unchecked(source, Cow::Owned(buf))
        }))
    }
}
