//! Regression tests for stream termination.
//!
//! The `MeasureManyStream` must properly terminate (return `None`) when all
//! ping requests have been sent and all responses have been received. Without
//! this, `while let Some(...) = stream.next().await` loops hang forever.

use std::{
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use futures_util::StreamExt;
use massping::{DualstackPinger, V4Pinger};
use tokio::time;

/// Test that the stream properly terminates after receiving all responses.
///
/// This is a regression test for the bug where `poll_next_unpin` always
/// returned `Poll::Pending` after processing all results, causing
/// `while let` loops to hang indefinitely.
#[tokio::test(flavor = "current_thread")]
async fn stream_terminates_after_single_ping() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    let mut stream = pinger.measure_many([localhost].into_iter());

    let mut count = 0;

    // This should complete - not hang forever
    let result = time::timeout(Duration::from_secs(5), async {
        while let Some((addr, rtt)) = stream.next().await {
            assert_eq!(addr, localhost);
            assert!(rtt < Duration::from_secs(1), "RTT too high: {rtt:?}");
            count += 1;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "stream did not terminate - hung in while let loop"
    );
    assert_eq!(count, 1, "expected exactly 1 ping response");
}

/// Test that the stream properly terminates after receiving multiple responses.
///
/// Note: We use different addresses because replies are matched by source
/// address, so duplicate addresses yield a single measurement per round.
#[tokio::test(flavor = "current_thread")]
async fn stream_terminates_after_multiple_pings() {
    let pinger = DualstackPinger::new().unwrap();

    // Use different loopback addresses (127.0.0.x all route to localhost)
    let addresses: Vec<IpAddr> = vec![
        "127.0.0.1".parse().unwrap(),
        "127.0.0.2".parse().unwrap(),
        "127.0.0.3".parse().unwrap(),
    ];
    let mut stream = pinger.measure_many(addresses.iter().copied());

    let mut count = 0;

    let result = time::timeout(Duration::from_secs(5), async {
        while let Some((_addr, rtt)) = stream.next().await {
            assert!(rtt < Duration::from_secs(1), "RTT too high: {rtt:?}");
            count += 1;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "stream did not terminate - hung in while let loop"
    );
    assert_eq!(count, 3, "expected exactly 3 ping responses");
}

/// Test that duplicate addresses yield a single measurement and terminate.
///
/// Replies are matched by source address within a round, so a duplicate
/// can never yield a second measurement; it is only pinged once.
#[tokio::test(flavor = "current_thread")]
async fn duplicate_addresses_yield_single_measurement() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    let mut stream = pinger.measure_many([localhost, localhost, localhost].into_iter());

    let mut count = 0;

    let result = time::timeout(Duration::from_secs(5), async {
        while stream.next().await.is_some() {
            count += 1;
        }
    })
    .await;

    assert!(result.is_ok(), "stream did not terminate");
    assert_eq!(count, 1, "duplicates should collapse into one measurement");
}

/// Test that the stream terminates when sends fail immediately.
///
/// `sendto` fails synchronously with `EACCES` for the broadcast address
/// (the socket doesn't have `SO_BROADCAST`), so no reply can ever arrive.
/// The stream must not wait for one forever.
#[tokio::test(flavor = "current_thread")]
async fn stream_terminates_when_send_fails() {
    let pinger = V4Pinger::new().unwrap();

    let addresses: [Ipv4Addr; 1] = ["255.255.255.255".parse().unwrap()];
    let mut stream = pinger.measure_many(addresses.into_iter());

    let mut count = 0;

    let result = time::timeout(Duration::from_secs(2), async {
        while stream.next().await.is_some() {
            count += 1;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "stream did not terminate after failed sends"
    );
    assert_eq!(count, 0, "expected no responses for unsendable addresses");
}

/// Test that an empty address list terminates immediately.
#[tokio::test(flavor = "current_thread")]
async fn stream_terminates_with_empty_input() {
    let pinger = DualstackPinger::new().unwrap();
    let addresses: Vec<IpAddr> = vec![];
    let mut stream = pinger.measure_many(addresses.into_iter());

    let result = time::timeout(Duration::from_secs(1), async {
        let first = stream.next().await;
        assert!(first.is_none(), "expected None for empty address list");
    })
    .await;

    assert!(result.is_ok(), "stream did not terminate for empty input");
}

/// Test stream termination with multi_thread runtime as a baseline.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_terminates_multi_thread() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    let mut stream = pinger.measure_many([localhost].into_iter());

    let mut count = 0;

    let result = time::timeout(Duration::from_secs(5), async {
        while let Some((addr, rtt)) = stream.next().await {
            assert_eq!(addr, localhost);
            assert!(rtt < Duration::from_secs(1), "RTT too high: {rtt:?}");
            count += 1;
        }
    })
    .await;

    assert!(
        result.is_ok(),
        "stream did not terminate - hung in while let loop"
    );
    assert_eq!(count, 1, "expected exactly 1 ping response");
}
