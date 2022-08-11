use std::{
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
    time::Duration,
};

use massping::{
    packet::EchoRequestPacket,
    raw_pinger::{RawV4Pinger, RawV6Pinger},
};
use tokio::time;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let localhost_v4: Ipv4Addr = "127.0.0.1".parse().unwrap();
    let one_one_one_one_v4: Ipv4Addr = "1.1.1.1".parse().unwrap();
    let not_answering_v4: Ipv4Addr = "0.0.0.1".parse().unwrap();
    let localhost_v6: Ipv6Addr = "::1".parse().unwrap();
    let one_one_one_one_v6: Ipv6Addr = "2606:4700:4700::1111".parse().unwrap();

    let v4_pinger = Arc::new(RawV4Pinger::new().expect("setup Ipv4 pinger"));
    let v6_pinger = Arc::new(RawV6Pinger::new().expect("setup Ipv6 pinger"));

    {
        let v4_pinger = Arc::clone(&v4_pinger);
        let v6_pinger = Arc::clone(&v6_pinger);

        tokio::spawn(async move {
            for addr in [localhost_v4, one_one_one_one_v4, not_answering_v4] {
                let mut payload = [0; 256];
                getrandom::getrandom(&mut payload).unwrap();

                let packet = EchoRequestPacket::new(1, 1, &payload);
                println!("Send ICMP v4 ping to {}", addr);
                v4_pinger
                    .send_to(addr, &packet)
                    .await
                    .expect("send v4 ping");

                //println!("Payload: {:?}", &packet[packet.len() - 256..]);
            }
        });

        tokio::spawn(async move {
            for addr in [localhost_v6, one_one_one_one_v6] {
                let mut payload = [0; 256];
                getrandom::getrandom(&mut payload).unwrap();

                let packet = EchoRequestPacket::new(1, 1, &payload);
                println!("Send ICMP v6 ping to {}", addr);
                v6_pinger
                    .send_to(addr, &packet)
                    .await
                    .expect("send v6 ping");

                //println!("Payload: {:?}", &packet[packet.len() - 256..]);
            }
        });
    }

    let handle_v4 = tokio::spawn(async move {
        loop {
            let packet = v4_pinger.recv().await.unwrap();
            println!(
                "Recv: {} | Identifier: {} | Sequence number {}",
                packet.source(),
                packet.identifier(),
                packet.sequence_number()
            );
            //println!("Payload: {:?}", &packet[packet.len() - 256..]);
        }
    });

    let handle_v6 = tokio::spawn(async move {
        loop {
            let packet = v6_pinger.recv().await.unwrap();
            println!(
                "Recv: {} | Identifier: {} | Sequence number {}",
                packet.source(),
                packet.identifier(),
                packet.sequence_number()
            );
            //println!("Payload: {:?}", &packet[packet.len() - 256..]);
        }
    });

    time::sleep(Duration::from_secs(5)).await;
    handle_v4.abort();
    handle_v6.abort();
}
