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
thread_local! { static ROOM: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }
fn hello(
    ws: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    version: u32,
) {
    ws.send(Message::text(
        serde_json::to_string(&ClientMessage::Hello {
            version,
            name: "test".into(),
            room: ROOM.with(|c| c.borrow().clone()),
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
            let message: ServerMessage = serde_json::from_str(&s).unwrap();
            if matches!(message, ServerMessage::Welcome { .. }) {
                loop {
                    if let Message::Text(text) = ws.read().unwrap()
                        && let ServerMessage::Lobby(lobby) = serde_json::from_str(&text).unwrap()
                    {
                        ROOM.with(|c| *c.borrow_mut() = Some(lobby.code));
                        break;
                    }
                }
            }
            return message;
        }
    }
}
#[test]
fn upgrade_allows_only_exact_development_origins_and_native_no_origin() {
    use tungstenite::client::IntoClientRequest;
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        tx.send(server.local_addr()).unwrap();
        while !flag.load(Ordering::Relaxed) {
            server.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    let address = format!("ws://{}", rx.recv().unwrap());
    for origin in [
        "https://evil.example",
        "http://localhost:9999",
        "http://localhost.evil:8000",
        "null",
    ] {
        let mut request = address.clone().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("Origin", origin.parse().unwrap());
        let result = connect(request);
        assert!(
            matches!(result, Err(tungstenite::Error::Http(ref response)) if response.status() == 403),
            "accepted {origin}"
        );
    }
    for origin in [
        Some("http://127.0.0.1:8000"),
        Some("http://localhost:8000"),
        None,
    ] {
        let mut request = address.clone().into_client_request().unwrap();
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("Origin", origin.parse().unwrap());
        }
        ROOM.with(|c| *c.borrow_mut() = None);
        let (mut socket, _) = connect(request).unwrap();
        hello(&mut socket, VERSION);
        assert!(matches!(read(&mut socket), ServerMessage::Welcome { .. }));
        socket.close(None).unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}

// Raw masked wire frames: tungstenite's public message API cannot express
// endless non-FIN continuations, which are the actual attacker input here.
fn raw_upgrade(address: std::net::SocketAddr) -> std::net::TcpStream {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(stream, "GET / HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }
    assert!(response.starts_with(b"HTTP/1.1 101"));
    stream
}
fn wire_text(stream: &mut std::net::TcpStream, text: &str) {
    use std::io::Write;
    assert!(text.len() < 126);
    let mut frame = vec![0x81, 0x80 | text.len() as u8, 0, 0, 0, 0];
    frame.extend_from_slice(text.as_bytes());
    stream.write_all(&frame).unwrap();
}
#[test]
fn fragmented_hello_trickle_expires_absolute_admission_deadline() {
    use std::io::Write;
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        tx.send(server.local_addr()).unwrap();
        while !flag.load(Ordering::Relaxed) {
            server.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    let addr = rx.recv().unwrap();
    let mut attackers: Vec<_> = (0..8).map(|_| raw_upgrade(addr)).collect();
    for s in &mut attackers {
        s.write_all(&[0x01, 0x80, 0, 0, 0, 0]).unwrap();
    }
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(11) {
        for s in &mut attackers {
            let _ = s.write_all(&[0x00, 0x80, 0, 0, 0, 0]);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let result = connect(format!("ws://{addr}"));
    assert!(
        result.is_ok(),
        "eight fragmented Hello tricklers permanently occupied admission workers: {result:?}"
    );
    let (mut healthy, _) = result.unwrap();
    hello(&mut healthy, VERSION);
    assert!(matches!(read(&mut healthy), ServerMessage::Welcome { .. }));
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}

#[test]
fn admitted_fragment_trickle_is_disconnected_while_healthy_ticks_advance() {
    use std::io::{Read, Write};
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        tx.send(server.local_addr()).unwrap();
        while !flag.load(Ordering::Relaxed) {
            server.tick();
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    let addr = rx.recv().unwrap();
    let mut attacker = raw_upgrade(addr);
    wire_text(
        &mut attacker,
        r#"{"Hello":{"version":2,"name":"fragmenter"}}"#,
    );
    std::thread::sleep(Duration::from_millis(100));
    // Drain Welcome/state before beginning an incomplete message.
    attacker.set_nonblocking(true).unwrap();
    while attacker.read(&mut [0; 8192]).is_ok() {}
    let (mut healthy, _) = connect(format!("ws://{addr}")).unwrap();
    hello(&mut healthy, VERSION);
    assert!(matches!(
        read(&mut healthy),
        ServerMessage::Welcome { seat: 0, .. }
    ));
    healthy.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    while !matches!(read(&mut healthy), ServerMessage::State(_)) {}
    attacker.write_all(&[0x01, 0x80, 0, 0, 0, 0]).unwrap();
    let start = std::time::Instant::now();
    let mut closed = false;
    let mut frames = 0;
    while start.elapsed() < Duration::from_secs(2) {
        let _ = attacker.write_all(&[0x00, 0x80, 0, 0, 0, 0]);
        loop {
            match attacker.read(&mut [0; 65536]) {
                Ok(0) => {
                    closed = true;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    closed = true;
                    break;
                }
                _ => {}
            }
        }
        if let ServerMessage::State(_) = read(&mut healthy) {
            frames += 1;
        }
    }
    assert!(frames > 10, "healthy gameplay starved");
    assert!(
        closed,
        "admitted fragmented message can live forever by trickling empty continuations"
    );
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
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
    delayed.command(ClientMessage::Ready(true)).unwrap();
    let start = std::time::Instant::now();
    while delayed.frame.as_ref().is_none_or(|f| f.ack[0] == 0) {
        delayed.pump(Default::default());
        assert!(start.elapsed() < Duration::from_secs(5));
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
    a.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    b.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    while !matches!(read(&mut b), ServerMessage::State(_)) {}
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
    ROOM.with(|c| *c.borrow_mut() = None);
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
    let mut client = ryggattack::net_client::NetworkClient::connect_room(
        &addr,
        "native adapter",
        ROOM.with(|c| c.borrow().clone()).as_deref(),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while client.seat.is_none() || client.lobby.is_none() {
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
    assert_eq!(client.lobby.as_ref().unwrap().humans.len(), 2);
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
