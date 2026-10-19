use ryggattack::{
    headless::{Simulation, SimulationConfig},
    input::ControlIntent,
};

#[test]
fn held_intent_obeys_half_second_cooldown_on_exact_ticks() {
    let mut simulation = Simulation::new(SimulationConfig {
        bots: 0,
        ..Default::default()
    })
    .unwrap();
    let before = simulation.snapshot();
    simulation
        .submit(
            0,
            ControlIntent {
                fire: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        simulation.snapshot(),
        before,
        "submission is not a simulation tick"
    );
    simulation.step();
    assert_eq!(simulation.snapshot().carts[0].shot_cooldown, 0.5);
    for _ in 0..31 {
        simulation.step();
    }
    assert_eq!(simulation.snapshot().carts[0].shot_cooldown, 0.015625);
    simulation.step();
    assert_eq!(simulation.snapshot().carts[0].shot_cooldown, 0.5);
    simulation.submit(0, ControlIntent::default()).unwrap();
    for _ in 0..32 {
        simulation.step();
    }
    assert_eq!(
        simulation.snapshot().carts[0].shot_cooldown,
        0.0,
        "neutral intent suppresses fire without engaging a bot"
    );
}

#[test]
fn bot_count_is_capped_by_external_humans_not_added_on_top() {
    let mut simulation = Simulation::new(SimulationConfig {
        human_seats: [true; 4],
        ..Default::default()
    })
    .unwrap();
    let snapshot = simulation.snapshot();
    assert_eq!(snapshot.carts.len(), 4);
    assert!(snapshot.carts.iter().all(|cart| cart.active && cart.alive));
    assert!(simulation.submit(3, ControlIntent::default()).is_ok());
}

#[test]
fn seed_replays_pickup_randomness_after_the_first_thirty_seconds() {
    let config = SimulationConfig {
        bots: 0,
        ..Default::default()
    };
    let mut left = Simulation::new(config.clone()).unwrap();
    let mut right = Simulation::new(config).unwrap();
    for tick in 1..=2048 {
        left.step();
        right.step();
        if tick % 64 == 0 {
            assert_eq!(left.snapshot(), right.snapshot(), "tick {tick}");
        }
    }
    assert_eq!(left.snapshot().pickups, 1);
}

#[test]
fn rejects_unsupported_match_configuration_before_startup() {
    for config in [
        SimulationConfig {
            bots: 4,
            ..Default::default()
        },
        SimulationConfig {
            wins_target: 0,
            ..Default::default()
        },
        SimulationConfig {
            wins_target: 2,
            ..Default::default()
        },
        SimulationConfig {
            human_seats: [false; 4],
            bots: 0,
            ..Default::default()
        },
    ] {
        assert!(Simulation::new(config.clone()).is_err(), "{config:?}");
    }
}

#[test]
fn rejects_invalid_or_uncontrolled_input_without_changing_state() {
    let mut simulation = Simulation::new(SimulationConfig::default()).unwrap();
    let before = simulation.snapshot();
    for (seat, horizontal, vertical) in [
        (1, 0.0, 0.0),
        (4, 0.0, 0.0),
        (usize::MAX, 0.0, 0.0),
        (0, f32::NAN, 0.0),
        (0, 0.0, f32::INFINITY),
        (0, f32::NEG_INFINITY, 0.0),
        (0, 1.01, 0.0),
        (0, 0.0, -1.01),
    ] {
        assert!(
            simulation
                .submit(
                    seat,
                    ControlIntent {
                        horizontal,
                        vertical,
                        fire: true
                    }
                )
                .is_err(),
            "seat={seat} horizontal={horizontal} vertical={vertical}"
        );
        assert_eq!(simulation.snapshot(), before);
    }
    simulation.step();
    assert_eq!(
        simulation.snapshot().projectiles,
        0,
        "invalid fire must not be retained"
    );
}
