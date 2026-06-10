#![cfg(feature = "stream")]

//! Regression test for sequence number wraparound.
//!
//! Rounds are identified on the wire by a `u16` sequence number, which
//! wraps around after 65536 `measure_many` calls. A long-lived round
//! dropped *after* the wraparound used to unsubscribe whichever newer
//! round had taken over its sequence number, leaving that round unable
//! to ever receive its replies.

use std::{iter, net::Ipv4Addr, time::Duration};

use futures_util::StreamExt;
use massping::V4Pinger;
use tokio::time;

#[tokio::test(flavor = "current_thread")]
async fn unsubscribe_after_wraparound_does_not_break_new_round() {
    let pinger = V4Pinger::new().unwrap();

    // Round with sequence number 0, kept alive across the wraparound.
    let stale = pinger.measure_many(iter::empty::<Ipv4Addr>());

    // Burn through the remaining 65535 sequence numbers.
    for _ in 0..65535 {
        drop(pinger.measure_many(iter::empty::<Ipv4Addr>()));
    }

    // This round wraps around to sequence number 0, taking over the slot.
    let mut current = pinger.measure_many([Ipv4Addr::LOCALHOST].into_iter());

    // Dropping the stale round must not unsubscribe the current one.
    drop(stale);

    let result = time::timeout(Duration::from_secs(5), current.next()).await;
    match result {
        Ok(Some((addr, _rtt))) => assert_eq!(addr, Ipv4Addr::LOCALHOST),
        Ok(None) => panic!("stream ended without a reply"),
        Err(_) => panic!("no reply: the stale unsubscribe broke the current round"),
    }
}
