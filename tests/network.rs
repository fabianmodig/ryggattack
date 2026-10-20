use ryggattack::network::{ClientMessage, VERSION, decode_client};
use ryggattack::{
    headless::{Simulation, SimulationConfig},
    input::ControlIntent,
};

#[test]
fn replication_includes_explicit_map_and_stable_visual_ids_and_powerup_positions() {
    let mut sim = Simulation::new(SimulationConfig {
        human_seats: [true; 4],
        bots: 0,
        ..Default::default()
    })
    .unwrap();
    let edges = sim.network_edges();
    assert!(edges.len() >= 24);
    let first = sim.network_frame();
    assert_eq!(
        serde_json::to_value(&first).unwrap()["simulation_tick"],
        0,
        "replication exposes the paused simulation clock separately from service ticks"
    );
    assert_eq!(first.carts.len(), 4);
    assert!(first.bodies.iter().any(|b| b.kind == 3));
    sim.submit(
        0,
        ControlIntent {
            fire: true,
            ..Default::default()
        },
    )
    .unwrap();
    sim.step();
    let second = sim.network_frame();
    assert!(second.bodies.iter().any(|b| b.kind == 0));
    let prop = first.bodies.iter().find(|b| b.kind == 3).unwrap();
    assert!(
        second
            .bodies
            .iter()
            .any(|b| b.id == prop.id && b.position == prop.position)
    );
    sim.submit(0, ControlIntent::default()).unwrap();
    for _ in 0..1920 {
        sim.step();
    }
    let frame = sim.network_frame();
    assert!(frame.bodies.iter().any(|b| b.kind == 1 || b.kind == 2));
    assert!(serde_json::to_string(&frame).unwrap().len() < ryggattack::network::MAX_STATE_BYTES);
}

#[test]
fn credentials_never_appear_in_debug_diagnostics() {
    let secret = "b".repeat(64);
    let claim = ryggattack::network::ClientMessage::Reconnect {
        version: VERSION,
        room: "a".repeat(20),
        participant: 1,
        credential: secret.clone(),
    };
    assert!(!format!("{claim:?}").contains(&secret));
    let welcome = ryggattack::network::ServerMessage::Welcome {
        version: VERSION,
        room: 1,
        participant: 1,
        seat: 0,
        edges: vec![],
        code: "a".repeat(20),
        credential: secret.clone(),
    };
    assert!(!format!("{welcome:?}").contains(&secret));
}
#[test]
fn reconnect_requires_private_bounded_credential_and_current_version() {
    let msg = serde_json::json!({"Reconnect":{"version":VERSION,"room":"a".repeat(20),"participant":1,"credential":"b".repeat(64)}});
    assert!(decode_client(&msg.to_string()).is_ok());
    for (key, value) in [
        ("credential", serde_json::json!("a".repeat(20))),
        ("participant", serde_json::json!(0)),
        ("version", serde_json::json!(0)),
    ] {
        let mut invalid = msg.clone();
        invalid["Reconnect"][key] = value;
        assert!(decode_client(&invalid.to_string()).is_err());
    }
}
#[test]
fn protocol_rejects_unknown_fields_nonfinite_axes_and_versions() {
    assert!(
        decode_client(
            r#"{"Input":{"sequence":1,"horizontal":0,"vertical":0,"fire":true,"seat":3}}"#
        )
        .is_err()
    );
    assert!(
        decode_client(r#"{"Input":{"sequence":1,"horizontal":2,"vertical":0,"fire":false}}"#)
            .is_err()
    );
    assert!(
        decode_client(&format!(
            r#"{{"Hello":{{"version":{},"name":"Ada"}}}}"#,
            VERSION + 1
        ))
        .is_err()
    );
    assert!(decode_client(&"x".repeat(1025)).is_err());
    for axes in ["NaN", "1e400", "-1.01"] {
        assert!(
            decode_client(&format!(
                r#"{{"Input":{{"sequence":1,"horizontal":{axes},"vertical":0,"fire":false}}}}"#
            ))
            .is_err()
        );
    }
    for name in ["", "\n", "abcdefghijklmnopqrstuvwxyz"] {
        let text = serde_json::to_string(&ClientMessage::Hello {
            version: VERSION,
            name: name.into(),
            room: None,
        })
        .unwrap();
        assert!(decode_client(&text).is_err());
    }
    assert!(matches!(
        decode_client(r#"{"Input":{"sequence":1,"horizontal":0,"vertical":-1,"fire":true}}"#),
        Ok(ClientMessage::Input { sequence: 1, .. })
    ));
}
