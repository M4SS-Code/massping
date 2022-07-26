use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use futures_util::StreamExt;
use massping::V4Pinger;
use tokio::time;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let localhost_v4: Ipv4Addr = "127.0.0.1".parse().unwrap();
    let one_one_one_one_v4: Ipv4Addr = "1.1.1.1".parse().unwrap();
    let not_answering_v4: Ipv4Addr = "0.0.0.1".parse().unwrap();

    let v4_pinger = Arc::new(V4Pinger::new().expect("setup Ipv4 pinger"));

    let ips = [localhost_v4, one_one_one_one_v4, not_answering_v4];

    let mut interval = time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;

        let v4_pinger = Arc::clone(&v4_pinger);
        tokio::spawn(async move {
            let _ = time::timeout(Duration::from_secs(5), async {
                let mut stream = v4_pinger.measure_many(ips.into_iter());
                while let Some((addr, took)) = stream.next().await {
                    println!("{}: {:?}", addr, took);
                }
            })
            .await;
        });
    }
}
