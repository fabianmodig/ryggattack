//! Deterministic real-framing regression at the running-seat authority boundary.
use super::*;
use tungstenite::protocol::Role;

fn sockets() -> (WebSocket<Transport>, WebSocket<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (stream, _) = listener.accept().unwrap();
    client.set_nodelay(true).unwrap();
    stream.set_nodelay(true).unwrap();
    stream.set_nonblocking(true).unwrap();
    (
        WebSocket::from_raw_socket(
            Transport {
                stream,
                deadline: None,
                quota: None,
                wire: Some(WireProgress::default()),
                http_matched: 0,
            },
            Role::Server,
            None,
        ),
        WebSocket::from_raw_socket(client, Role::Client, None),
    )
}
fn arrived(room: &Room, bytes: usize) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut buffer = [0; 512];
        match room.peers[0]
            .as_ref()
            .unwrap()
            .socket
            .get_ref()
            .stream
            .peek(&mut buffer)
        {
            Ok(n) if n >= bytes => return,
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("local frame arrival: {e}"),
        }
        assert!(
            Instant::now() < deadline,
            "complete masked frame must arrive before advancing the room"
        );
        std::thread::yield_now();
    }
}
#[test]
fn running_reclaim_discards_queued_old_input_and_leave_before_new_owner_controls_same_cart() {
    for queue_leave in [false, true] {
        let now = Instant::now();
        let mut room = Room {
            lobby: crate::rooms::Lobby::new(42, "0123456789abcdef0123".into(), 1, "host".into()),
            peers: Default::default(),
            simulation: None,
            victory_ticks: 0,
            empty_since: None,
            reservations: [None; 4],
            credentials: Default::default(),
            last_activity: now,
        };
        let (socket, mut old) = sockets();
        room.attach(socket, 0, 1, now);
        let credential = room.credentials[0].clone().unwrap();
        room.lobby.ready(1, true).unwrap();
        for tick in 1..=128 {
            room.tick_at(tick, now);
        }
        assert_eq!(room.lobby.phase, crate::rooms::Phase::Running);
        let baseline = r#"{"Input":{"sequence":1,"horizontal":-1,"vertical":0,"fire":false}}"#;
        old.send(Message::text(baseline)).unwrap();
        arrived(&room, baseline.len() + 6);
        room.tick_at(129, now);
        assert_eq!(room.peers[0].as_ref().unwrap().sequence, 1);
        assert_eq!(
            room.simulation
                .as_ref()
                .unwrap()
                .app
                .world()
                .resource::<crate::input::SeatInputs>()
                .intents[0]
                .horizontal,
            -1.0,
            "old connection genuinely controlled a RUNNING cart before replacement"
        );

        let stale = r#"{"Input":{"sequence":999999,"horizontal":1,"vertical":0,"fire":true}}"#;
        old.send(Message::text(stale)).unwrap();
        let mut pending_bytes = stale.len() + 6;
        if queue_leave {
            old.send(Message::text("\"Leave\"")).unwrap();
            pending_bytes += 7 + 6;
        }
        arrived(&room, pending_bytes);
        let before = room.simulation.as_mut().unwrap().snapshot();
        let (seat, rotated) = room.reclaim(1, &credential, now).unwrap();
        assert_eq!(seat, 0);
        assert!(rotated != credential, "private bearer must rotate");
        assert!(
            room.peers[0].is_none(),
            "old socket and its queued parser bytes must be gone before attach"
        );
        assert_eq!(
            room.simulation.as_mut().unwrap().snapshot(),
            before,
            "claim neither respawns nor moves the cart"
        );
        assert_eq!(
            room.simulation
                .as_ref()
                .unwrap()
                .app
                .world()
                .resource::<crate::input::SeatInputs>()
                .intents[0],
            ControlIntent::default(),
            "claim immediately neutralizes the previous owner's held intent"
        );
        assert!(
            room.reclaim(1, &credential, now).is_err(),
            "replay must fail without changing the new authority"
        );

        let (socket, mut current) = sockets();
        room.attach(socket, 0, 1, now);
        for sequence in [1, 2] {
            let intent = ControlIntent {
                horizontal: 0.0,
                vertical: 1.0,
                fire: false,
            };
            let message = serde_json::to_string(&ClientMessage::Input {
                sequence,
                horizontal: intent.horizontal,
                vertical: intent.vertical,
                fire: intent.fire,
            })
            .unwrap();
            current.send(Message::text(message.clone())).unwrap();
            arrived(&room, message.len() + 6);
            room.tick_at(129 + sequence, now);
            assert_eq!(
                room.peers[0].as_ref().unwrap().sequence,
                sequence,
                "old high sequence must not poison the replacement"
            );
            assert_eq!(
                room.simulation
                    .as_ref()
                    .unwrap()
                    .app
                    .world()
                    .resource::<crate::input::SeatInputs>()
                    .intents[0],
                intent,
                "only the replacement's actual decoded input has authority"
            );
            assert!(
                room.lobby.humans.iter().any(|h| h.id == 1 && h.connected),
                "queued old Leave must not evict the replacement"
            );
            let world = room.simulation.as_mut().unwrap().app.world_mut();
            let (wants_to_fire, shot_cooldown) = world
                .query::<&crate::players::Player>()
                .iter(world)
                .find(|p| p.id == 0)
                .map(|p| (p.wants_to_fire, p.shot_cooldown))
                .expect("the same human cart remains present");
            assert!(
                !wants_to_fire && shot_cooldown == 0.0,
                "buffered old fire must not create a shot or start its cooldown"
            );
            assert_eq!(room.lobby.phase, crate::rooms::Phase::Running);
        }
    }
}
