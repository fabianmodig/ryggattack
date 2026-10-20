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
fn until(s: &mut Socket, key: &str) -> serde_json::Value {
    for _ in 0..400 {
        if let Message::Text(t) = s.read().unwrap() {
            let v: serde_json::Value = serde_json::from_str(&t).unwrap();
            if let Some(value) = v.get(key) {
                return value.clone();
            }
        }
    }
    panic!("missing {key}")
}
fn dial(addr: &str, msg: serde_json::Value) -> Socket {
    let (mut s, _) = tungstenite::connect(addr).unwrap();
    if let MaybeTlsStream::Plain(t) = s.get_mut() {
        t.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    }
    s.send(Message::text(msg.to_string())).unwrap();
    s
}
fn hello(code: Option<&str>) -> serde_json::Value {
    serde_json::json!({"Hello":{"version":VERSION,"name":"same name","room":code}})
}
fn claim(w: &serde_json::Value, code: &str) -> serde_json::Value {
    serde_json::json!({"Reconnect":{"version":VERSION,"room":code,"participant":w["participant"],"credential":w["credential"]}})
}
#[test]
fn real_sockets_reconnect_rotate_reject_cross_room_and_supersede_old_input_and_leave() {
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
    let mut incompatible = dial(
        &addr,
        serde_json::json!({"Hello":{"version":VERSION+1,"name":"old"}}),
    );
    assert!(
        until(&mut incompatible, "Error")
            .as_str()
            .unwrap()
            .contains("incompatible")
    );
    let mut a = dial(&addr, hello(None));
    let aw = until(&mut a, "Welcome");
    assert!(
        aw["credential"]
            .as_str()
            .is_some_and(ryggattack::network::valid_credential)
    );
    let al = until(&mut a, "Lobby");
    let code = al["code"].as_str().unwrap();
    let mut b = dial(&addr, hello(Some(code)));
    let bw = until(&mut b, "Welcome");
    let mut other = dial(&addr, hello(None));
    until(&mut other, "Welcome");
    let ol = until(&mut other, "Lobby");
    let mut bad = dial(&addr, claim(&aw, ol["code"].as_str().unwrap()));
    assert!(
        until(&mut bad, "Error")
            .as_str()
            .unwrap()
            .contains("reconnect")
    );
    let mut restored = dial(&addr, claim(&aw, code));
    let rw = until(&mut restored, "Welcome");
    assert_eq!(aw["participant"], rw["participant"]);
    assert_eq!(aw["seat"], rw["seat"]);
    assert!(
        aw["credential"] != rw["credential"],
        "credential rotates privately"
    );
    let _ = a.send(Message::text("\"Leave\""));
    let _ = a.send(Message::text(
        r#"{"Input":{"sequence":999999,"horizontal":1,"vertical":0,"fire":true}}"#,
    ));
    let mut replay = dial(&addr, claim(&aw, code));
    assert!(
        until(&mut replay, "Error")
            .as_str()
            .unwrap()
            .contains("reconnect")
    );
    restored.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    b.send(Message::text(r#"{"Ready":true}"#)).unwrap();
    for _ in 0..200 {
        if until(&mut b, "Lobby")["phase"] == "Running" {
            break;
        }
    }
    let f = until(&mut b, "State");
    assert_eq!(f["ack"][0], 0, "stale socket input has no authority");
    drop(restored);
    let mut saw = false;
    for _ in 0..100 {
        let l = until(&mut b, "Lobby");
        if l["host"] == bw["participant"] {
            assert_eq!(l["humans"].as_array().unwrap().len(), 2);
            assert_eq!(l["humans"][0]["connected"], false);
            saw = true;
            break;
        }
    }
    assert!(saw);
    let mut again = dial(&addr, claim(&rw, code));
    let final_w = until(&mut again, "Welcome");
    assert_eq!(final_w["participant"], aw["participant"]);
    let l = until(&mut again, "Lobby");
    assert_eq!(l["host"], bw["participant"], "former host remains ordinary");
    again
        .send(Message::text(
            r#"{"Input":{"sequence":1,"horizontal":0,"vertical":0,"fire":false}}"#,
        ))
        .unwrap();
    let mut ack = false;
    for _ in 0..100 {
        if until(&mut again, "State")["ack"][0] == 1 {
            ack = true;
            break;
        }
    }
    assert!(ack, "new socket resets sequence");
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
}
