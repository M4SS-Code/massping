use std::{net::IpAddr, sync::Arc, time::Duration};

use futures_util::StreamExt;
use massping::DualstackPinger;
use tokio::time;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let localhost_v4: IpAddr = "127.0.0.1".parse().unwrap();
    let one_one_one_one_v4: IpAddr = "1.1.1.1".parse().unwrap();
    let not_answering_v4: IpAddr = "0.0.0.1".parse().unwrap();
    let localhost_v6: IpAddr = "::1".parse().unwrap();
    let one_one_one_one_v6: IpAddr = "2606:4700:4700::1111".parse().unwrap();

    let pinger = Arc::new(DualstackPinger::new().expect("setup pinger"));

    let ips = [
        localhost_v4,
        one_one_one_one_v4,
        not_answering_v4,
        localhost_v6,
        one_one_one_one_v6,
    ];

    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;

        let pinger = Arc::clone(&pinger);
        tokio::spawn(async move {
            let _ = time::timeout(Duration::from_secs(5), async {
                let mut stream = pinger.measure_many(ips.into_iter());
                while let Some((addr, took)) = stream.next().await {
                    println!("{}: {:?}", addr, took);
                }
            })
            .await;
        });
    }
}
