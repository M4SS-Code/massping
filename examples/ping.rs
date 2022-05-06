use std::time::Duration;

use massping::ping;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let localhost_v4 = "127.0.0.1".parse().unwrap();
    let one_one_one_one_v4 = "1.1.1.1".parse().unwrap();
    let not_answering_v4 = "0.0.0.1".parse().unwrap();
    let localhost_v6 = "::1".parse().unwrap();
    let one_one_one_one_v6 = "2606:4700:4700::1111".parse().unwrap();

    let addrs = [
        localhost_v4,
        one_one_one_one_v4,
        not_answering_v4,
        localhost_v6,
        one_one_one_one_v6,
    ];
    let rtt = Duration::from_secs(5);
    let size = 64;
    let pings = ping(addrs.into_iter(), rtt, size).await.unwrap();

    println!("{:?}", pings)
}
