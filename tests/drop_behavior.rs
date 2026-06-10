//! Regression test for resource cleanup on `Pinger` drop.
//!
//! The background receive task used to hold (through `Arc<InnerPinger>`)
//! the only sender of its own subscription channel, so the channel never
//! disconnected, the task never exited and the ICMP socket was never
//! closed. Every `Pinger` created and dropped leaked a task and a file
//! descriptor for the lifetime of the runtime.

use massping::V4Pinger;

fn count_fds() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_pinger_closes_socket_and_stops_task() {
    // Warm up anything created lazily (runtime resources, RNG, ...) so the
    // baseline below is stable.
    drop(V4Pinger::new().unwrap());
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
    let baseline = count_fds();

    let pingers: Vec<_> = (0..5).map(|_| V4Pinger::new().unwrap()).collect();
    assert!(
        count_fds() >= baseline + 5,
        "each pinger should hold a socket fd"
    );
    drop(pingers);

    // The background tasks observe the subscription channel closing and
    // exit, dropping their sockets. Yield so the current_thread runtime
    // gets a chance to run them.
    let mut fds = count_fds();
    for _ in 0..100 {
        if fds == baseline {
            break;
        }
        tokio::task::yield_now().await;
        fds = count_fds();
    }

    assert_eq!(
        fds, baseline,
        "background task/socket leaked after dropping the pingers"
    );
}
