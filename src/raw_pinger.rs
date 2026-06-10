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
///
/// The underlying socket remembers at most one waker per direction, so at
/// most one task should be sending and one task receiving at any given
/// time; concurrent polls in the same direction can lose wakeups.
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
    ///
    /// Replies larger than 2048 bytes are silently truncated.
    pub fn recv(&self) -> RecvFuture<'_, V> {
        RecvFuture {
            pinger: self,
            buf: BytesMut::new(),
        }
    }

    /// Receive an ICMP ECHO reply packet
    ///
    /// Replies larger than 2048 bytes are silently truncated.
    pub fn poll_recv(
        &self,
        buf: &mut BytesMut,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<EchoReplyPacket<V>>> {
        let (buf, source) = ready!(self.socket.poll_read(buf, cx))?;
        // Skip packets that don't come from the expected address family or
        // aren't valid echo replies, and ask to be polled again: more
        // packets may already be queued on the socket.
        let packet = V::from_ip_addr(source.ip())
            .and_then(|source| EchoReplyPacket::from_reply(source, buf));
        match packet {
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
        Poll::Ready(Ok(packet))
    }
}

#[cfg(test)]
mod tests {
    use std::{future::poll_fn, net::Ipv4Addr, time::Duration};

    use bytes::BytesMut;
    use tokio::time::timeout;

    use super::RawPinger;
    use crate::packet::EchoRequestPacket;

    /// Test that verifies `poll_recv` doesn't accumulate data in the buffer
    /// across multiple calls.
    ///
    /// We test this by using `poll_recv` directly with a shared buffer across
    /// multiple ping/recv cycles and verifying the buffer state.
    #[tokio::test]
    async fn poll_recv_clears_buffer_between_calls() {
        let pinger: RawPinger<Ipv4Addr> = RawPinger::new().unwrap();
        let mut recv_buf = BytesMut::new();

        for i in 0..3u16 {
            let packet = EchoRequestPacket::new(0x1234, i, b"test payload here");
            pinger.send_to(Ipv4Addr::LOCALHOST, &packet).await.unwrap();

            // Use poll_recv directly so we can inspect the buffer
            let result = timeout(
                Duration::from_secs(5),
                poll_fn(|cx| pinger.poll_recv(&mut recv_buf, cx)),
            )
            .await;

            match result {
                Ok(Ok(reply)) => {
                    assert_eq!(reply.source(), Ipv4Addr::LOCALHOST);
                    assert_eq!(reply.sequence_number(), i);

                    // buffer should be empty after successful recv
                    // because poll_read calls buf.split().freeze()
                    assert!(
                        recv_buf.is_empty(),
                        "Buffer should be empty, but has {} bytes on iteration {i}",
                        recv_buf.len()
                    );
                }
                Ok(Err(e)) => panic!("recv {i} failed with error: {e}"),
                Err(_) => panic!("timeout on recv {i}"),
            }
        }
    }

    /// Test that multiple sequential receives work correctly.
    #[tokio::test]
    async fn multiple_sequential_receives() {
        let pinger: RawPinger<Ipv4Addr> = RawPinger::new().unwrap();

        for i in 0..3u16 {
            let packet = EchoRequestPacket::new(0x1234, i, b"test");
            pinger.send_to(Ipv4Addr::LOCALHOST, &packet).await.unwrap();

            let result = timeout(Duration::from_secs(5), pinger.recv()).await;

            match result {
                Ok(Ok(reply)) => {
                    assert_eq!(reply.source(), Ipv4Addr::LOCALHOST);
                    assert_eq!(reply.sequence_number(), i);
                }
                Ok(Err(e)) => panic!("recv {i} failed with error: {e}"),
                Err(_) => panic!("timeout on recv {i}"),
            }
        }
    }
}
