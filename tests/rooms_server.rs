#![cfg(not(target_arch = "wasm32"))]
use ryggattack::{network::VERSION, server::Server};
use std::{
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};
type Socket = WebSocket<MaybeTlsStream<TcpStream>>;
fn read(s: &mut Socket) -> serde_json::Value {
    loop {
        if let Message::Text(t) = s.read().unwrap() {
            return serde_json::from_str(&t).unwrap();
        }
    }
}
fn until(s: &mut Socket, key: &str) -> serde_json::Value {
    for _ in 0..400 {
        let v = read(s);
        if v.get(key).is_some() {
            return v[key].clone();
        }
    }
    panic!("missing {key}")
}
fn join(addr: &str, code: Option<&str>) -> (Socket, serde_json::Value) {
    let (mut s, _) = tungstenite::connect(addr).unwrap();
    if let MaybeTlsStream::Plain(t) = s.get_mut() {
        t.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    }
    s.send(Message::text(
        serde_json::json!({"Hello":{"version":VERSION,"name":"same name","room":code}}).to_string(),
    ))
    .unwrap();
    let w = until(&mut s, "Welcome");
    (s, w)
}
#[test]
fn actual_rooms_isolate_ids_rosters_permissions_countdown_and_running_admission() {
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let worker = std::thread::spawn(move || {
        let mut server = Server::bind("127.0.0.1:0").unwrap();
        tx.send(server.local_addr()).unwrap();
        while !flag.load(Ordering::Relaxed) {
            server.tick();
            std::thread::sleep(Duration::from_millis(4));
        }
    });
    let addr = format!("ws://{}", rx.recv().unwrap());
    let (mut a, aw) = join(&addr, None);
    let lobby = until(&mut a, "Lobby");
    let code = lobby["code"].as_str().unwrap();
    assert_eq!(code.len(), 20);
    let (mut b, bw) = join(&addr, Some(code));
    assert_ne!(aw["participant"], bw["participant"]);
    assert_eq!(aw["room"], bw["room"]);
    let (mut c, cw) = join(&addr, None);
    assert_ne!(cw["room"], aw["room"]);
    let separate = until(&mut c, "Lobby");
    assert_eq!(separate["humans"].as_array().unwrap().len(), 1);
    b.send(Message::text(
        r#"{"Configure":{"arena":"Volcano","wins_target":1,"bots":1}}"#,
    ))
    .unwrap();
    assert!(until(&mut b, "Notice").as_str().unwrap().contains("host"));
    a.send(Message::text(
        r#"{"Configure":{"arena":"Volcano","wins_target":1,"bots":1}}"#,
    ))
    .unwrap();
    a.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    b.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    let mut saw_countdown = false;
    for _ in 0..200 {
        let l = until(&mut a, "Lobby");
        if l["phase"].get("Countdown").is_some() {
            saw_countdown = true;
        }
        if l["phase"] == "Running" {
            break;
        }
    }
    assert!(saw_countdown);
    let f = until(&mut a, "State");
    assert_eq!(
        f["carts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["active"] == true)
            .count(),
        3
    );
    let (mut rejected, _) = tungstenite::connect(&addr).unwrap();
    rejected
        .send(Message::text(
            serde_json::json!({"Hello":{"version":VERSION,"name":"new","room":code}}).to_string(),
        ))
        .unwrap();
    assert!(
        until(&mut rejected, "Error")
            .as_str()
            .unwrap()
            .contains("running")
    );
    a.send(Message::text(r#""Leave""#)).unwrap();
    let mut elected = false;
    for _ in 0..200 {
        let l = until(&mut b, "Lobby");
        if l["humans"].as_array().unwrap().len() == 1 {
            assert_eq!(l["host"], bw["participant"]);
            elected = true;
            break;
        }
    }
    assert!(
        elected,
        "remaining participant never observed host election"
    );
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
}
