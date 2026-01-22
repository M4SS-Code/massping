//! Regression tests for stream termination.
//!
//! The `MeasureManyStream` must properly terminate (return `None`) when all
//! ping requests have been sent and all responses have been received. Without
//! this, `while let Some(...) = stream.next().await` loops hang forever.

use std::{net::IpAddr, time::Duration};

use futures_util::StreamExt;
use massping::DualstackPinger;
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
/// Note: We use different addresses because `in_flight` is keyed by address,
/// so pinging the same address multiple times in one `measure_many` call
/// would overwrite the previous entry.
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
