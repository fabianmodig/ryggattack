use ryggattack::{
    headless::{Arena, Simulation, SimulationConfig},
    input::ControlIntent,
    network::{Frame, MAX_STATE_BYTES},
};

#[test]
fn visual_descriptors_survive_the_wire_for_both_arenas_and_all_owners() {
    for arena in [Arena::Forest, Arena::Volcano] {
        let mut sim = Simulation::new(SimulationConfig {
            arena,
            human_seats: [true; 4],
            bots: 0,
            ..Default::default()
        })
        .unwrap();
        let first = sim.network_frame();
        let bytes = serde_json::to_vec(&first).unwrap();
        assert!(bytes.len() < MAX_STATE_BYTES);
        let wire: Frame = serde_json::from_slice(&bytes).unwrap();
        let value = serde_json::to_value(&wire).unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        let mut variants = std::collections::BTreeSet::new();
        for body in value["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["kind"] == 3)
        {
            assert!(
                body["prop"].is_object(),
                "live prop descriptor missing: {body}"
            );
            kinds.insert(body["prop"]["kind"].as_str().unwrap().to_owned());
            variants.insert(body["prop"]["variant"].as_u64().unwrap());
            let scale = body["prop"]["scale"].as_f64().unwrap();
            assert!(scale > 0.0);
            if arena == Arena::Volcano {
                assert_eq!(body["prop"]["kind"], "Rock");
                assert!(scale <= 1.5);
            }
        }
        assert_eq!(variants, [0, 1].into());
        if arena == Arena::Forest {
            assert_eq!(
                kinds,
                ["Pine", "Birch", "Bush", "Rock", "Stump", "Fern"]
                    .map(str::to_owned)
                    .into()
            );
        }
    }
}

#[test]
fn projectile_owner_seats_survive_the_wire_for_both_arenas() {
    for arena in [Arena::Forest, Arena::Volcano] {
        let mut sim = Simulation::new(SimulationConfig {
            arena,
            human_seats: [true; 4],
            bots: 0,
            ..Default::default()
        })
        .unwrap();
        for seat in 0..4 {
            sim.submit(
                seat,
                ControlIntent {
                    fire: true,
                    ..Default::default()
                },
            )
            .unwrap();
        }
        sim.step();
        let frame = sim.network_frame();
        let bytes = serde_json::to_vec(&frame).unwrap();
        assert!(bytes.len() < MAX_STATE_BYTES);
        let wire: Frame = serde_json::from_slice(&bytes).unwrap();
        let value = serde_json::to_value(&wire).unwrap();
        let owners: std::collections::BTreeSet<_> = value["bodies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| b["kind"] == 0)
            .map(|b| b["owner"].as_u64().expect("projectile owner missing"))
            .collect();
        assert_eq!(owners, [0, 1, 2, 3].into());
    }
}
