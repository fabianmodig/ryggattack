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
        })
        .unwrap();
        assert!(decode_client(&text).is_err());
    }
    assert!(matches!(
        decode_client(r#"{"Input":{"sequence":1,"horizontal":0,"vertical":-1,"fire":true}}"#),
        Ok(ClientMessage::Input { sequence: 1, .. })
    ));
}
