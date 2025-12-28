use std::{
    collections::HashMap,
    io,
    iter::Peekable,
    net::{Ipv4Addr, Ipv6Addr},
    sync::{
        Arc,
        atomic::{AtomicU16, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};
#[cfg(feature = "stream")]
use std::{pin::Pin, task::ready};

#[cfg(feature = "stream")]
use futures_core::Stream;
use tokio::{
    sync::mpsc::{self, error::TryRecvError},
    time::Instant,
};

use crate::{IpVersion, packet::EchoRequestPacket, raw_pinger::RawPinger};

/// A pinger for IPv4 addresses
pub type V4Pinger = Pinger<Ipv4Addr>;
/// A pinger for IPv6 addresses
pub type V6Pinger = Pinger<Ipv6Addr>;

/// A pinger for [`IpVersion`] (either [`Ipv4Addr`] or [`Ipv6Addr`]).
pub struct Pinger<V: IpVersion> {
    inner: Arc<InnerPinger<V>>,
}

struct InnerPinger<V: IpVersion> {
    raw: RawPinger<V>,
    round_sender: mpsc::UnboundedSender<RoundMessage<V>>,
    identifier: u16,
    sequence_number: AtomicU16,
}

enum RoundMessage<V: IpVersion> {
    Subscribe {
        sequence_number: u16,
        sender: mpsc::UnboundedSender<(V, Instant)>,
    },
    Unsubscribe {
        sequence_number: u16,
    },
}

impl<V: IpVersion> Pinger<V> {
    /// Construct a new `Pinger`.
    ///
    /// For maximum efficiency the same instance of `Pinger` should
    /// be used for as long as possible, altough it might also
    /// be beneficial to `Drop` the `Pinger` and recreate it if
    /// you are not going to be sending pings for a long period of time.
    pub fn new() -> io::Result<Self> {
        let raw = RawPinger::new()?;

        let mut identifier = [0; 2];
        getrandom::fill(&mut identifier).expect("run getrandom");
        let identifier = u16::from_ne_bytes(identifier);

        let (sender, mut receiver) = mpsc::unbounded_channel();

        let inner = Arc::new(InnerPinger {
            raw,
            round_sender: sender,
            identifier,
            sequence_number: AtomicU16::new(0),
        });

        // Spawn async receive task using the same socket
        let inner_recv = Arc::clone(&inner);
        tokio::spawn(async move {
            let mut subscribers: HashMap<u16, mpsc::UnboundedSender<(V, Instant)>> = HashMap::new();

            loop {
                // Process any pending subscription changes
                loop {
                    match receiver.try_recv() {
                        Ok(RoundMessage::Subscribe {
                            sequence_number,
                            sender,
                        }) => {
                            subscribers.insert(sequence_number, sender);
                        }
                        Ok(RoundMessage::Unsubscribe { sequence_number }) => {
                            drop(subscribers.remove(&sequence_number));
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }

                // Receive next packet (with DGRAM sockets, kernel handles routing)
                let packet = match inner_recv.raw.recv().await {
                    Ok(packet) => packet,
                    Err(_) => continue,
                };

                let recv_instant = Instant::now();

                let packet_source = packet.source();
                let packet_sequence_number = packet.sequence_number();

                if let Some(subscriber) = subscribers.get(&packet_sequence_number) {
                    if subscriber.send((packet_source, recv_instant)).is_err() {
                        subscribers.remove(&packet_sequence_number);
                    }
                }
            }
        });

        Ok(Self { inner })
    }

    /// Ping `addresses`
    ///
    /// Creates [`MeasureManyStream`] which **lazily** sends ping
    /// requests and [`Stream`]s the responses as they arrive.
    ///
    /// [`Stream`]: futures_core::Stream
    pub fn measure_many<I>(&self, addresses: I) -> MeasureManyStream<'_, V, I>
    where
        I: Iterator<Item = V>,
    {
        let (size_hint, _) = addresses.size_hint();
        let send_queue = addresses.into_iter().peekable();
        let (sender, receiver) = mpsc::unbounded_channel();

        let sequence_number = self.inner.sequence_number.fetch_add(1, Ordering::AcqRel);
        if self
            .inner
            .round_sender
            .send(RoundMessage::Subscribe {
                sequence_number,
                sender,
            })
            .is_err()
        {
            panic!("Receiver closed");
        }

        MeasureManyStream {
            pinger: self,
            send_queue,
            in_flight: HashMap::with_capacity(size_hint),
            receiver,
            sequence_number,
        }
    }
}

/// A [`Stream`] of ping responses.
///
/// No kind of `rtt` timeout is implemented, so an external mechanism
/// like [`tokio::time::timeout`] should be used to prevent the program
/// from hanging indefinitely.
///
/// Leaking this method might crate a slowly forever growing memory leak.
///
/// [`Stream`]: futures_core::Stream
/// [`tokio::time::timeout`]: tokio::time::timeout
pub struct MeasureManyStream<'a, V: IpVersion, I: Iterator<Item = V>> {
    pinger: &'a Pinger<V>,
    send_queue: Peekable<I>,
    in_flight: HashMap<V, Instant>,
    receiver: mpsc::UnboundedReceiver<(V, Instant)>,
    sequence_number: u16,
}

impl<V: IpVersion, I: Iterator<Item = V>> MeasureManyStream<'_, V, I> {
    pub fn poll_next_unpin(&mut self, cx: &mut Context<'_>) -> Poll<(V, Duration)> {
        // Try to see if another `MeasureManyStream` got it
        if let Poll::Ready(Some((addr, rtt))) = self.poll_next_from_different_round(cx) {
            return Poll::Ready((addr, rtt));
        }

        // Try to send ICMP echo requests
        self.poll_next_icmp_replies(cx);

        Poll::Pending
    }

    fn poll_next_icmp_replies(&mut self, cx: &mut Context<'_>) {
        while let Some(&addr) = self.send_queue.peek() {
            let mut payload = [0; 64];
            getrandom::fill(&mut payload).expect("generate random payload");

            let packet = EchoRequestPacket::new(
                self.pinger.inner.identifier,
                self.sequence_number,
                &payload,
            );
            match self.pinger.inner.raw.poll_send_to(cx, addr, &packet) {
                Poll::Ready(_) => {
                    let sent_at = Instant::now();

                    let taken_addr = self.send_queue.next();
                    debug_assert!(taken_addr.is_some());

                    self.in_flight.insert(addr, sent_at);
                }
                Poll::Pending => break,
            }
        }
    }

    fn poll_next_from_different_round(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<(V, Duration)>> {
        loop {
            match self.receiver.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some((addr, recv_instant))) => {
                    if let Some(send_instant) = self.in_flight.remove(&addr) {
                        let rtt = recv_instant - send_instant;
                        return Poll::Ready(Some((addr, rtt)));
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
            }
        }
    }
}

#[cfg(feature = "stream")]
impl<V: IpVersion, I: Iterator<Item = V> + Unpin> Stream for MeasureManyStream<'_, V, I> {
    type Item = (V, Duration);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = ready!(self.as_mut().poll_next_unpin(cx));
        Poll::Ready(Some(result))
    }
}

impl<V: IpVersion, I: Iterator<Item = V>> Drop for MeasureManyStream<'_, V, I> {
    fn drop(&mut self) {
        let _ = self
            .pinger
            .inner
            .round_sender
            .send(RoundMessage::Unsubscribe {
                sequence_number: self.sequence_number,
            });
    }
}
