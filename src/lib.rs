//! A simplified version of [fastping-rs](https://github.com/bparli/fastping-rs)
//! without some of its [issues](https://github.com/bparli/fastping-rs/issues/25).
//!
//! Depends on the tokio 1 runtime.
//!
//! Tested on: Linux
//!
//! As with the original version, this one also requires to create raw sockets,
//! so the permission must either be explicitly set
//! (`sudo setcap cap_net_raw=+ep /path/to/binary` for example) or be run as root.

#[cfg(feature = "strong")]
use std::time::Instant;
use std::{
    collections::HashMap,
    io,
    iter::Peekable,
    mem::MaybeUninit,
    net::{Ipv4Addr, Ipv6Addr},
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};
#[cfg(feature = "stream")]
use std::{pin::Pin, task::ready};

#[cfg(feature = "stream")]
use futures_core::Stream;
use tokio::{
    sync::mpsc::{
        self,
        error::{TryRecvError, TrySendError},
    },
    task,
};

#[cfg(not(feature = "strong"))]
use self::instant::Instant;
pub use self::ip_version::IpVersion;
use self::{
    packet::{EchoReplyPacket, EchoRequestPacket},
    pinger::{RawBlockingPinger, RawPinger},
};

pub type V4Pinger = Pinger<Ipv4Addr>;
pub type V6Pinger = Pinger<Ipv6Addr>;

#[cfg(not(feature = "strong"))]
mod instant;
mod ip_version;
pub mod packet;
pub mod pinger;
mod socket;

pub struct Pinger<V: IpVersion> {
    inner: Arc<InnerPinger<V>>,
}

struct InnerPinger<V: IpVersion> {
    raw: RawPinger<V>,
    round_sender: mpsc::Sender<RoundMessage<V>>,
    identifier: u16,
    sequence_number: AtomicU16,
}

enum RoundMessage<V: IpVersion> {
    Subscribe {
        sequence_number: u16,
        #[cfg(feature = "strong")]
        sender: mpsc::Sender<(V, Instant)>,
        #[cfg(not(feature = "strong"))]
        sender: mpsc::Sender<(V, Instant, Instant)>,
    },
    Unsubscribe {
        sequence_number: u16,
    },
}

impl<V: IpVersion> Pinger<V> {
    pub fn new() -> io::Result<Self> {
        let raw = RawPinger::new()?;
        let raw_blocking = RawBlockingPinger::new()?;

        let mut identifier = [0; 2];
        getrandom::getrandom(&mut identifier).expect("run getrandom");
        let identifier = u16::from_ne_bytes(identifier);

        let (sender, mut receiver) = mpsc::channel(8);

        let inner = Arc::new(InnerPinger {
            raw,
            round_sender: sender,
            identifier,
            sequence_number: AtomicU16::new(0),
        });
        task::spawn_blocking(move || {
            let mut buf = [MaybeUninit::<u8>::uninit(); 1600];

            #[cfg(feature = "strong")]
            let mut subscribers: HashMap<u16, mpsc::Sender<(V, Instant)>> = HashMap::new();
            #[cfg(not(feature = "strong"))]
            let mut subscribers: HashMap<u16, mpsc::Sender<(V, Instant, Instant)>> = HashMap::new();
            'packets: while let Ok(packet) = raw_blocking.recv(&mut buf) {
                if packet.identifier() != identifier {
                    continue 'packets;
                }

                let recv_instant = Instant::now();

                #[cfg(not(feature = "strong"))]
                let send_instant = {
                    let payload = packet.payload();
                    match Instant::decode(payload[..Instant::ENCODED_LEN].try_into().unwrap()) {
                        Some(send_instant) => send_instant,
                        None => continue 'packets,
                    }
                };

                let packet_source = packet.source();
                let packet_sequence_number = packet.sequence_number();
                match subscribers.get(&packet_sequence_number) {
                    Some(subscriber) => {
                        #[cfg(feature = "strong")]
                        if let Err(TrySendError::Closed(_)) =
                            subscriber.try_send((packet_source, recv_instant))
                        {
                            // Closed
                            subscribers.remove(&packet_sequence_number);
                        }

                        #[cfg(not(feature = "strong"))]
                        if let Err(TrySendError::Closed(_)) =
                            subscriber.try_send((packet_source, send_instant, recv_instant))
                        {
                            // Closed
                            subscribers.remove(&packet_sequence_number);
                        }
                    }
                    None => 'registrations: loop {
                        match receiver.try_recv() {
                            Ok(RoundMessage::Subscribe {
                                sequence_number,
                                sender,
                            }) => {
                                if packet_sequence_number == sequence_number {
                                    // Packet matches

                                    #[cfg(feature = "strong")]
                                    if sender.try_send((packet_source, recv_instant)).is_err() {
                                        // Closed
                                        continue 'registrations;
                                    }

                                    #[cfg(not(feature = "strong"))]
                                    if sender
                                        .try_send((packet_source, send_instant, recv_instant))
                                        .is_err()
                                    {
                                        // Closed
                                        continue 'registrations;
                                    }
                                }

                                subscribers.insert(sequence_number, sender);
                            }
                            Ok(RoundMessage::Unsubscribe { sequence_number }) => {
                                drop(subscribers.remove(&sequence_number));
                            }
                            Err(TryRecvError::Empty) => {
                                break 'registrations;
                            }
                            Err(TryRecvError::Disconnected) => {
                                break 'packets;
                            }
                        }
                    },
                }
            }
        });

        Ok(Self { inner })
    }

    pub fn measure_many<I>(&self, addresses: I) -> MeasureManyStream<'_, V, I>
    where
        I: Iterator<Item = V>,
    {
        let send_queue = addresses.into_iter().peekable();
        let (sender, receiver) = mpsc::channel(32);

        let sequence_number = self.inner.sequence_number.fetch_add(1, Ordering::AcqRel);
        if self
            .inner
            .round_sender
            .try_send(RoundMessage::Subscribe {
                sequence_number,
                sender,
            })
            .is_err()
        {
            panic!("Couldn't register sender");
        }

        MeasureManyStream {
            pinger: self,
            send_queue,
            #[cfg(feature = "strong")]
            in_flight: HashMap::new(),
            receiver,
            sequence_number,
        }
    }
}

pub struct MeasureManyStream<'a, V: IpVersion, I: Iterator<Item = V>> {
    pinger: &'a Pinger<V>,
    send_queue: Peekable<I>,
    #[cfg(feature = "strong")]
    in_flight: HashMap<V, Instant>,
    #[cfg(feature = "strong")]
    receiver: mpsc::Receiver<(V, Instant)>,
    #[cfg(not(feature = "strong"))]
    receiver: mpsc::Receiver<(V, Instant, Instant)>,
    sequence_number: u16,
}

impl<'a, V: IpVersion, I: Iterator<Item = V>> MeasureManyStream<'a, V, I> {
    pub fn poll_next_unpin(&mut self, cx: &mut Context) -> Poll<(V, Duration)> {
        // Try to see if another `MeasureManyStream` got it
        if let Poll::Ready(Some((addr, rtt))) = self.poll_next_from_different_round(cx) {
            return Poll::Ready((addr, rtt));
        }

        // Try to send ICMP echo requests
        self.poll_next_icmp_replys(cx);

        Poll::Pending
    }

    fn poll_next_icmp_replys(&mut self, cx: &mut Context) {
        while let Some(&addr) = self.send_queue.peek() {
            let mut payload = [0; 64];
            #[cfg(feature = "strong")]
            getrandom::getrandom(&mut payload).expect("generate random payload");
            #[cfg(not(feature = "strong"))]
            {
                let now = Instant::now().encode();
                let (now_part, random_part) = payload.split_at_mut(now.len());
                now_part.copy_from_slice(&now);
                getrandom::getrandom(random_part).expect("generate random payload");
            }

            let packet = EchoRequestPacket::new(
                self.pinger.inner.identifier,
                self.sequence_number,
                &payload,
            );
            match self.pinger.inner.raw.poll_send_to(cx, addr, &packet) {
                Poll::Ready(_) => {
                    #[cfg(feature = "strong")]
                    let sent_at = Instant::now();

                    let taken_addr = self.send_queue.next();
                    debug_assert!(taken_addr.is_some());

                    #[cfg(feature = "strong")]
                    self.in_flight.insert(addr, sent_at);
                }
                Poll::Pending => break,
            }
        }
    }

    #[cfg_attr(not(feature = "strong"), allow(clippy::never_loop))]
    fn poll_next_from_different_round(&mut self, cx: &mut Context) -> Poll<Option<(V, Duration)>> {
        loop {
            match self.receiver.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                #[cfg(feature = "strong")]
                Poll::Ready(Some((addr, recv_instant))) => {
                    if let Some(send_instant) = self.in_flight.remove(&addr) {
                        let rtt = recv_instant - send_instant;
                        return Poll::Ready(Some((addr, rtt)));
                    }
                }
                #[cfg(not(feature = "strong"))]
                Poll::Ready(Some((addr, send_instant, recv_instant))) => {
                    let rtt = recv_instant - send_instant;
                    return Poll::Ready(Some((addr, rtt)));
                }
                Poll::Ready(None) => return Poll::Ready(None),
            }
        }
    }
}

#[cfg(feature = "stream")]
impl<'a, V: IpVersion, I: Iterator<Item = V> + Unpin> Stream for MeasureManyStream<'a, V, I> {
    type Item = (V, Duration);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = ready!(self.as_mut().poll_next_unpin(cx));
        Poll::Ready(Some(result))
    }
}

impl<'a, V: IpVersion, I: Iterator<Item = V>> Drop for MeasureManyStream<'a, V, I> {
    fn drop(&mut self) {
        let _ = self
            .pinger
            .inner
            .round_sender
            .try_send(RoundMessage::Unsubscribe {
                sequence_number: self.sequence_number,
            });
    }
}
