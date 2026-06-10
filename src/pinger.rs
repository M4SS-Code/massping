#[cfg(feature = "stream")]
use std::pin::Pin;
use std::{
    collections::HashMap,
    future::poll_fn,
    io,
    iter::Peekable,
    net::{Ipv4Addr, Ipv6Addr},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use bytes::BytesMut;
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
    // Kept out of `InnerPinger` (which the background receive task holds)
    // so that dropping the `Pinger` disconnects the channel, telling the
    // background task to shut down and release the socket.
    round_sender: mpsc::UnboundedSender<RoundMessage<V>>,
}

struct InnerPinger<V: IpVersion> {
    raw: RawPinger<V>,
    identifier: u16,
    next_round_id: AtomicU64,
}

// Each `measure_many` round gets a unique `u64` id; the wire sequence
// number is its lower 16 bits. The full id lets the receive task tell
// rounds apart after the sequence number wraps around.
enum RoundMessage<V: IpVersion> {
    Subscribe {
        round_id: u64,
        sender: mpsc::UnboundedSender<(V, Instant)>,
    },
    Unsubscribe {
        round_id: u64,
    },
}

struct Subscriber<V: IpVersion> {
    round_id: u64,
    sender: mpsc::UnboundedSender<(V, Instant)>,
}

enum PollResult<V: IpVersion> {
    Subscription(RoundMessage<V>),
    Packet(crate::packet::EchoReplyPacket<V>),
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

        let identifier = rand::random::<u16>();

        let (sender, mut receiver) = mpsc::unbounded_channel();

        let inner = Arc::new(InnerPinger {
            raw,
            identifier,
            next_round_id: AtomicU64::new(0),
        });

        // Spawn async receive task using the same socket.
        // It runs until `receiver` disconnects, which happens when the
        // `Pinger` holding the only sender is dropped.
        let inner_recv = Arc::clone(&inner);
        tokio::spawn(async move {
            let mut subscribers: HashMap<u16, Subscriber<V>> = HashMap::new();
            // Buffer kept outside poll_fn so it persists across polls.
            let mut recv_buf = BytesMut::new();

            loop {
                // Poll both subscription channel and socket in the same waker context.
                // This ensures we wake on either event, which is required for
                // single-threaded runtimes where we can't rely on concurrent execution.
                //
                // Note: We use try_recv() before poll_recv() as a fast path optimization.
                // Benchmarks show this is ~2x faster when messages are already queued
                // (~15ns vs ~25ns per iteration).
                let result = poll_fn(|cx| {
                    // Fast path: check for subscription changes (non-blocking, no waker)
                    match receiver.try_recv() {
                        Ok(msg) => return Poll::Ready(Some(PollResult::Subscription(msg))),
                        Err(TryRecvError::Empty) => {
                            // Continue - poll_recv() below will register the waker for this channel
                        }
                        Err(TryRecvError::Disconnected) => return Poll::Ready(None),
                    }

                    // Try to receive an ICMP packet
                    match inner_recv.raw.poll_recv(&mut recv_buf, cx) {
                        Poll::Ready(Ok(packet)) => {
                            return Poll::Ready(Some(PollResult::Packet(packet)));
                        }
                        Poll::Ready(Err(_)) => {
                            // Receiving failed (typically a transient kernel
                            // resource error). The socket readiness was
                            // consumed without registering a waker, so ask to
                            // be polled again right away; parking here would
                            // suspend reply processing until an unrelated
                            // subscription message wakes the task.
                            cx.waker().wake_by_ref();
                        }
                        Poll::Pending => {}
                    }

                    // Register waker for subscription channel
                    // We need to wake up when new subscriptions arrive
                    match receiver.poll_recv(cx) {
                        Poll::Ready(Some(msg)) => {
                            return Poll::Ready(Some(PollResult::Subscription(msg)));
                        }
                        Poll::Ready(None) => return Poll::Ready(None),
                        Poll::Pending => {}
                    }

                    Poll::Pending
                })
                .await;

                match result {
                    Some(PollResult::Subscription(RoundMessage::Subscribe {
                        round_id,
                        sender,
                    })) => {
                        // A new round may displace a still-subscribed round
                        // whose sequence number collided after wraparound;
                        // the displaced round could not be served anyway as
                        // replies can only be told apart by sequence number.
                        subscribers.insert(round_id as u16, Subscriber { round_id, sender });
                    }
                    Some(PollResult::Subscription(RoundMessage::Unsubscribe { round_id })) => {
                        let sequence_number = round_id as u16;
                        // Only unsubscribe if the slot still belongs to this
                        // round: after sequence number wraparound it may have
                        // been taken over by a newer round, which must keep
                        // receiving replies.
                        if subscribers
                            .get(&sequence_number)
                            .is_some_and(|subscriber| subscriber.round_id == round_id)
                        {
                            subscribers.remove(&sequence_number);
                        }
                    }
                    Some(PollResult::Packet(packet)) => {
                        let recv_instant = Instant::now();

                        let packet_source = packet.source();
                        let packet_sequence_number = packet.sequence_number();

                        if let Some(subscriber) = subscribers.get(&packet_sequence_number) {
                            if subscriber
                                .sender
                                .send((packet_source, recv_instant))
                                .is_err()
                            {
                                subscribers.remove(&packet_sequence_number);
                            }
                        }
                    }
                    None => return, // Channel closed
                }
            }
        });

        Ok(Self {
            inner,
            round_sender: sender,
        })
    }

    /// Ping `addresses`
    ///
    /// Creates [`MeasureManyStream`] which **lazily** sends ping
    /// requests and [`Stream`]s the responses as they arrive.
    ///
    /// Replies are matched by source address, so an address that appears
    /// multiple times is only pinged once per round and yields a single
    /// measurement.
    ///
    /// [`Stream`]: futures_core::Stream
    pub fn measure_many<I>(&self, addresses: I) -> MeasureManyStream<'_, V, I>
    where
        I: Iterator<Item = V>,
    {
        let (size_hint, _) = addresses.size_hint();
        let send_queue = addresses.into_iter().peekable();
        let (sender, receiver) = mpsc::unbounded_channel();

        // Relaxed is enough: the counter is a pure id allocator, no other
        // memory is synchronized through it.
        let round_id = self.inner.next_round_id.fetch_add(1, Ordering::Relaxed);
        if self
            .round_sender
            .send(RoundMessage::Subscribe { round_id, sender })
            .is_err()
        {
            panic!("Receiver closed");
        }

        MeasureManyStream {
            pinger: self,
            send_queue,
            in_flight: HashMap::with_capacity(size_hint),
            receiver,
            round_id,
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
    round_id: u64,
}

impl<V: IpVersion, I: Iterator<Item = V>> MeasureManyStream<'_, V, I> {
    pub fn poll_next_unpin(&mut self, cx: &mut Context<'_>) -> Poll<Option<(V, Duration)>> {
        // Try to receive a response (may be from a different round)
        if let Poll::Ready(maybe_reply) = self.poll_next_from_different_round(cx) {
            return Poll::Ready(maybe_reply);
        }

        // Try to send ICMP echo requests
        self.poll_next_icmp_replies(cx);

        // Check if we're done: no more addresses to send AND no responses pending
        if self.send_queue.peek().is_none() && self.in_flight.is_empty() {
            return Poll::Ready(None);
        }

        Poll::Pending
    }

    fn poll_next_icmp_replies(&mut self, cx: &mut Context<'_>) {
        while let Some(&addr) = self.send_queue.peek() {
            // Replies are matched by source address within a round, so a
            // second ping to an address that is still awaiting its reply
            // could never produce a second measurement; it would only
            // clobber the first ping's start time. Skip the duplicate.
            if self.in_flight.contains_key(&addr) {
                self.send_queue.next();
                continue;
            }

            let payload = rand::random::<[u8; 64]>();

            let packet = EchoRequestPacket::new(
                self.pinger.inner.identifier,
                self.round_id as u16,
                &payload,
            );
            match self.pinger.inner.raw.poll_send_to(cx, addr, &packet) {
                Poll::Ready(result) => {
                    let sent_at = Instant::now();

                    let taken_addr = self.send_queue.next();
                    debug_assert!(taken_addr.is_some());

                    // If the send failed (e.g. no route to host) no reply
                    // can ever arrive, so don't track the address as
                    // in-flight or the stream would never terminate.
                    if result.is_ok() {
                        self.in_flight.insert(addr, sent_at);
                    }
                }
                Poll::Pending => {
                    // The socket only remembers the most recent waker per
                    // direction (`AsyncFd` semantics), so with multiple
                    // streams sharing the socket another stream could
                    // overwrite ours and we'd never be woken again. Sends
                    // only return `Pending` while the send buffer is full,
                    // which clears up quickly, so schedule an immediate
                    // re-poll instead of parking.
                    cx.waker().wake_by_ref();
                    break;
                }
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
        self.as_mut().poll_next_unpin(cx)
    }
}

impl<V: IpVersion, I: Iterator<Item = V>> Drop for MeasureManyStream<'_, V, I> {
    fn drop(&mut self) {
        let _ = self.pinger.round_sender.send(RoundMessage::Unsubscribe {
            round_id: self.round_id,
        });
    }
}
