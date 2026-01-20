//! Regression test for GitHub issue #1:
//! `measure_many` hangs with `current_thread` tokio runtime.
//!
//! The bug was a race condition where the background receive task would
//! block on socket recv() before processing subscription messages. In a
//! single-threaded runtime, this caused ICMP replies to be dropped because
//! subscribers weren't registered yet.
//!
//! This test requires network access and the ability to send ICMP packets
//! to localhost. On Linux, this requires either:
//! - The process GID to be within `net.ipv4.ping_group_range` sysctl, OR
//! - Root privileges or `CAP_NET_RAW` capability

use std::{net::IpAddr, time::Duration};

use futures_util::StreamExt;
use massping::DualstackPinger;
use tokio::time;

/// Test that pinging localhost works with `current_thread` runtime.
///
/// This is a regression test for issue #1 where `measure_many` would hang
/// indefinitely on single-threaded runtimes due to a race condition between
/// subscription registration and ICMP reply processing.
#[tokio::test(flavor = "current_thread")]
async fn ping_localhost_current_thread() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    let mut stream = pinger.measure_many([localhost].into_iter());

    // With the bug, this would hang forever. With the fix, localhost should
    // respond within milliseconds. We use a generous 5 second timeout to
    // account for slow CI environments.
    let result = time::timeout(Duration::from_secs(5), stream.next()).await;

    match result {
        Ok(Some((addr, rtt))) => {
            assert_eq!(addr, localhost);
            // Localhost RTT should be very fast (sub-millisecond typically)
            assert!(rtt < Duration::from_secs(1), "RTT too high: {rtt:?}");
        }
        Ok(None) => {
            panic!("stream ended unexpectedly");
        }
        Err(_) => {
            panic!(
                "timeout waiting for ping response - \
                this indicates the current_thread runtime bug (issue #1) has regressed"
            );
        }
    }
}

/// Test that pinging localhost works with `multi_thread` runtime.
///
/// This serves as a baseline - if this test passes but `current_thread` fails,
/// it confirms the issue is specific to single-threaded runtimes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ping_localhost_multi_thread() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();
    let mut stream = pinger.measure_many([localhost].into_iter());

    let result = time::timeout(Duration::from_secs(5), stream.next()).await;

    match result {
        Ok(Some((addr, rtt))) => {
            assert_eq!(addr, localhost);
            assert!(rtt < Duration::from_secs(1), "RTT too high: {rtt:?}");
        }
        Ok(None) => {
            panic!("stream ended unexpectedly");
        }
        Err(_) => {
            panic!("timeout waiting for ping response");
        }
    }
}

/// Test pinging multiple times sequentially with `current_thread` runtime.
///
/// This tests that multiple sequential ping operations work correctly,
/// ensuring the fix handles repeated use of the pinger.
#[tokio::test(flavor = "current_thread")]
async fn ping_sequential_current_thread() {
    let pinger = DualstackPinger::new().unwrap();
    let localhost: IpAddr = "127.0.0.1".parse().unwrap();

    // Perform multiple sequential pings
    for i in 0..3 {
        let mut stream = pinger.measure_many([localhost].into_iter());

        let result = time::timeout(Duration::from_secs(5), stream.next()).await;

        match result {
            Ok(Some((addr, rtt))) => {
                assert_eq!(addr, localhost);
                assert!(
                    rtt < Duration::from_secs(1),
                    "RTT too high on ping {i}: {rtt:?}"
                );
            }
            Ok(None) => {
                panic!("stream ended unexpectedly on ping {i}");
            }
            Err(_) => {
                panic!(
                    "timeout on ping {i} - \
                    current_thread runtime bug may have regressed"
                );
            }
        }
    }
}
