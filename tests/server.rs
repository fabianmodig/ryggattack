#![cfg(not(target_arch = "wasm32"))]
use ryggattack::{
    network::{ClientMessage, ServerMessage, VERSION},
    server::Server,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tungstenite::{Message, connect};
fn hello(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    version: u32,
) {
    ws.send(Message::text(
        serde_json::to_string(&ClientMessage::Hello {
            version,
            name: "test".into(),
        })
        .unwrap(),
    ))
    .unwrap();
}
fn read(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
) -> ServerMessage {
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = ws.get_mut() {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
    }
    loop {
        if let Message::Text(s) = ws.read().unwrap() {
            return serde_json::from_str(&s).unwrap();
        }
    }
}
#[test]
fn service_refuses_unprotected_non_loopback_binding() {
    assert!(Server::bind("0.0.0.0:0").is_err());
}

#[test]
fn real_sockets_assign_distinct_seats_reject_invalid_input_and_release_disconnected_seat() {
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        tx.send(server.local_addr()).unwrap();
        while !flag.load(Ordering::Relaxed) {
            server.tick();
            std::thread::sleep(ryggattack::headless::TICK_DURATION);
        }
    });
    let addr = format!("ws://{}", rx.recv().unwrap());
    // A client that never completes HTTP upgrade must not stall gameplay/admission.
    let _half_open = std::net::TcpStream::connect(addr.trim_start_matches("ws://")).unwrap();
    // A renderer can stall before its first update. Admission must not expire
    // between socket-open and the adapter's first pump during scene startup.
    let mut delayed =
        ryggattack::net_client::NetworkClient::connect(&addr, "slow startup").unwrap();
    std::thread::sleep(Duration::from_secs(11));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while delayed.seat.is_none() {
        delayed.pump(Default::default());
        assert!(
            std::time::Instant::now() < deadline,
            "startup admission failed: {}",
            delayed.status
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    // Fresh acknowledged state can arrive while rendering is stalled. Consuming
    // it late must not be mistaken for a dead server solely due to send age.
    std::thread::sleep(Duration::from_secs(6));
    delayed.pump(Default::default());
    assert!(
        delayed.status.starts_with("Online"),
        "fresh state after render stall: {}",
        delayed.status
    );
    assert!(delayed.rtt_ms.is_some());
    drop(delayed);
    std::thread::sleep(Duration::from_millis(100));
    let (mut a, _) = connect(&addr).unwrap();
    hello(&mut a, VERSION);
    assert!(matches!(
        read(&mut a),
        ServerMessage::Welcome { seat: 0, .. }
    ));
    let (mut b, _) = connect(&addr).unwrap();
    hello(&mut b, VERSION);
    assert!(matches!(
        read(&mut b),
        ServerMessage::Welcome { seat: 1, .. }
    ));
    a.send(Message::text(
        r#"{"Input":{"sequence":7,"horizontal":0,"vertical":1,"fire":true}}"#,
    ))
    .unwrap();
    loop {
        if let ServerMessage::State(f) = read(&mut b)
            && f.ack[0] == 7
        {
            assert_eq!(f.ack[1], 0);
            break;
        }
    }
    // Replayed input cannot overwrite newer held intent or advance its ack.
    a.send(Message::text(
        r#"{"Input":{"sequence":6,"horizontal":-1,"vertical":0,"fire":false}}"#,
    ))
    .unwrap();
    for _ in 0..3 {
        if let ServerMessage::State(f) = read(&mut b) {
            assert_eq!(f.ack[0], 7);
        }
    }
    a.send(Message::text(
        r#"{"Input":{"sequence":8,"horizontal":2,"vertical":0,"fire":false}}"#,
    ))
    .unwrap();
    loop {
        if matches!(read(&mut a), ServerMessage::Error(_)) {
            break;
        }
    }
    drop(a);
    b.close(None).unwrap();
    drop(b);
    std::thread::sleep(Duration::from_millis(100));
    let (mut c, _) = connect(&addr).unwrap();
    hello(&mut c, VERSION + 1);
    assert!(matches!(read(&mut c), ServerMessage::Error(_)));
    drop(c);
    let (mut d, _) = connect(&addr).unwrap();
    hello(&mut d, VERSION);
    assert!(matches!(
        read(&mut d),
        ServerMessage::Welcome { seat: 0, .. }
    ));
    let mut client =
        ryggattack::net_client::NetworkClient::connect(&addr, "native adapter").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while client.seat.is_none() || client.frame.as_ref().is_none_or(|f| f.ack[1] == 0) {
        assert!(
            std::time::Instant::now() < deadline,
            "adapter handshake/input timed out: {}",
            client.status
        );
        client.pump(ryggattack::input::ControlIntent {
            fire: true,
            ..Default::default()
        });
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(client.seat, Some(1));
    assert!(client.rtt_ms.is_some());
    let (mut e, _) = connect(&addr).unwrap();
    hello(&mut e, VERSION);
    assert!(matches!(
        read(&mut e),
        ServerMessage::Welcome { seat: 2, .. }
    ));
    let (mut f, _) = connect(&addr).unwrap();
    hello(&mut f, VERSION);
    assert!(matches!(
        read(&mut f),
        ServerMessage::Welcome { seat: 3, .. }
    ));
    let (mut full, _) = connect(&addr).unwrap();
    hello(&mut full, VERSION);
    assert!(matches!(read(&mut full), ServerMessage::Error(_)));
    drop(client);
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}
