//! Runtime probe using exactly the same adapter as the graphical client.
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use ryggattack::{input::ControlIntent, net_client::NetworkClient};
    use std::time::{Duration, Instant};
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://127.0.0.1:8787".into());
    let seconds: u64 = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "15".into())
        .parse()
        .expect("seconds");
    let mut client = NetworkClient::connect(&address, "Native probe").expect("connect");
    let start = Instant::now();
    let mut tick = 0;
    let mut samples = Vec::new();
    while start.elapsed() < Duration::from_secs(seconds) {
        client.pump(ControlIntent {
            horizontal: if start.elapsed().as_secs().is_multiple_of(2) {
                1.
            } else {
                -1.
            },
            vertical: 0.,
            fire: true,
        });
        if let Some(frame) = &client.frame
            && frame.tick != tick
        {
            tick = frame.tick;
            if let Some(rtt) = client.rtt_ms {
                samples.push(rtt);
            }
            println!(
                "{}",
                serde_json::json!({"seat":client.seat,"frame":frame,"rtt_ms":client.rtt_ms})
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(tick > 0, "no authoritative state: {}", client.status);
    samples.sort_by(f64::total_cmp);
    eprintln!(
        "{}",
        serde_json::json!({"seat":client.seat,"last_tick":tick,"samples":samples.len(),"median_ack_rtt_ms":samples.get(samples.len()/2),"status":client.status})
    );
}
#[cfg(target_arch = "wasm32")]
fn main() {}
