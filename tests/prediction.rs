use ryggattack::headless::{Simulation, SimulationConfig};
use ryggattack::{input::ControlIntent, prediction::Movement};

#[test]
fn prediction_uses_shared_fixed_travel_before_new_authority() {
    let mut sim = Simulation::new(SimulationConfig {
        bots: 0,
        human_seats: [true, true, false, false],
        ..Default::default()
    })
    .unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let initial = sim.network_frame();
    movement.observe(&initial, 0.0, 0);
    let intent = ControlIntent {
        horizontal: 1.0,
        vertical: 1.0,
        fire: true,
    };
    for _ in 0..120 {
        movement.advance(1.0 / 64.0, intent);
        sim.submit(0, intent).unwrap();
        sim.step();
        // Refresh authority periodically but still exercise actual outstanding steps.
        let predicted = movement.local_position().unwrap();
        let authoritative = sim.snapshot().carts[0].position;
        for axis in 0..3 {
            assert!((predicted[axis] - authoritative[axis]).abs() < 1e-4);
        }
        let tick = sim.snapshot().tick;
        if tick.is_multiple_of(8) {
            movement.observe(&sim.network_frame(), tick as f64 / 64.0, 0);
        }
    }
}

#[test]
fn reconciliation_replays_actual_steps_not_acknowledged_messages() {
    let mut sim = Simulation::new(SimulationConfig {
        bots: 0,
        human_seats: [true, true, false, false],
        ..Default::default()
    })
    .unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    let right = ControlIntent {
        horizontal: 1.0,
        ..Default::default()
    };
    let up = ControlIntent {
        vertical: 1.0,
        ..Default::default()
    };
    for _ in 0..4 {
        movement.advance(1.0 / 64.0, right);
        sim.submit(0, right).unwrap();
        sim.step();
    }
    let mut acknowledged = sim.network_frame();
    // A held message has already been acknowledged, but future actual steps
    // using that same held intent must not be discarded on a delivery watermark.
    acknowledged.ack[0] = 999;
    for _ in 0..4 {
        movement.advance(1.0 / 64.0, up);
        sim.submit(0, up).unwrap();
        sim.step();
    }
    let expected = sim.snapshot().carts[0].position;
    movement.observe(&acknowledged, 0.125, 0);
    assert_eq!(
        movement.local_position().unwrap(),
        expected,
        "restore tick 4 then replay exactly steps 5..8"
    );
}

#[test]
fn a_long_render_stall_discards_speculation_instead_of_catching_up() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    movement.advance(10.0, Default::default());
    assert!(
        movement.local_position().is_none(),
        "fallback to authority after a long stall"
    );
}

#[test]
fn prediction_horizon_is_bounded_even_with_many_small_render_frames() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..240 {
        movement.advance(1.0 / 240.0, Default::default());
    }
    assert!(
        movement.local_position().is_none(),
        "a small delta cannot evade the half-second horizon"
    );
}

#[test]
fn stale_or_duplicate_frames_do_not_rewind_prediction() {
    let mut sim = Simulation::new(SimulationConfig {
        bots: 0,
        ..Default::default()
    })
    .unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let old = sim.network_frame();
    movement.observe(&old, 0.0, 0);
    for _ in 0..4 {
        sim.step();
        movement.advance(1.0 / 64.0, Default::default());
    }
    let fresh = sim.network_frame();
    movement.observe(&fresh, 0.0625, 0);
    for _ in 0..4 {
        movement.advance(1.0 / 64.0, Default::default());
    }
    let before = movement.local_position();
    movement.observe(&fresh, 0.125, 0);
    movement.observe(&old, 0.126, 0);
    assert_eq!(movement.local_position(), before);
}

#[test]
fn processed_input_reports_the_held_state_selected_at_a_completed_tick() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let right = ControlIntent {
        horizontal: 1.0,
        ..Default::default()
    };
    let up = ControlIntent {
        vertical: 1.0,
        ..Default::default()
    };
    sim.submit_sequenced(0, 2, right).unwrap();
    sim.submit_sequenced(0, 3, up).unwrap();
    assert_eq!(
        sim.network_frame().motion.unwrap().applied[0].sequence,
        0,
        "delivery is not completed simulation"
    );
    sim.step();
    for _ in 0..3 {
        sim.step();
    }
    let applied = sim.network_frame().motion.unwrap().applied[0];
    assert_eq!(
        applied.sequence, 3,
        "overwritten sequence 2 executes no tick"
    );
    assert_eq!(
        applied.since_tick, 1,
        "sequence 3 is held across four steps"
    );
    assert_eq!(applied.axes, [0.0, 1.0]);
    sim.submit(0, Default::default()).unwrap();
    sim.step();
    let neutral = sim.network_frame().motion.unwrap().applied[0];
    assert_eq!(
        neutral.sequence, 0,
        "server-generated neutral is not the old client command"
    );
    assert_eq!(neutral.since_tick, 5);
}

#[test]
fn rail_interpolation_follows_the_curve_and_never_cuts_the_corner() {
    use ryggattack::{
        network::{RailPath, RailState},
        prediction::interpolate_rails,
    };
    let from = RailState {
        path: RailPath::Junction {
            from: 11,
            node: 12,
            to: 7,
        },
        progress: 0.0,
        requested_turn: [0.0; 3],
    };
    let mut to = from.clone();
    to.progress = 1.0;
    let (position, forward) = interpolate_rails(&from, &to, 0.5).unwrap();
    let radius = ((position[0] + 0.9).powi(2) + (position[2] + 0.9).powi(2)).sqrt();
    assert!(
        (radius - 0.9).abs() < 1e-5,
        "must follow the actual arc rather than its chord"
    );
    assert!((forward[0].powi(2) + forward[2].powi(2) - 1.0).abs() < 1e-5);
    let edge = RailState {
        path: RailPath::Edge { from: 11, to: 12 },
        progress: 0.99,
        requested_turn: [0.0; 3],
    };
    to.progress = 0.1;
    let middle = interpolate_rails(&edge, &to, 0.5).unwrap().0;
    assert!(middle.iter().all(|v| v.is_finite()));
    let wrong = RailState {
        path: RailPath::Junction {
            from: 13,
            node: 12,
            to: 7,
        },
        ..to
    };
    assert!(
        interpolate_rails(&edge, &wrong, 0.5).is_none(),
        "route disagreement must snap, not cross unrelated rails"
    );
}

#[test]
fn remote_carts_move_between_samples_and_stop_without_extrapolation() {
    let mut sim = Simulation::new(SimulationConfig {
        bots: 0,
        human_seats: [true, true, false, false],
        ..Default::default()
    })
    .unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for tick in 1..=8 {
        sim.step();
        if tick % 4 == 0 {
            movement.observe(&sim.network_frame(), tick as f64 / 64.0, 0);
        }
    }
    let a = movement.remote_pose(1, 0.15625).unwrap().0;
    let b = movement.remote_pose(1, 0.171875).unwrap().0;
    assert_ne!(
        a, b,
        "no new snapshot is needed for a new remote render pose"
    );
    let final_pose = movement.remote_pose(1, 1.0).unwrap().0;
    assert_eq!(
        final_pose,
        sim.snapshot().carts[1].position,
        "stale remote samples hold at authority, never extrapolate forever"
    );
}

#[test]
fn a_death_respawn_boundary_cannot_replay_previous_life_inputs() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..8 {
        movement.advance(
            1.0 / 64.0,
            ControlIntent {
                horizontal: 1.0,
                ..Default::default()
            },
        );
    }
    for _ in 0..4 {
        sim.step();
    }
    let mut dead = sim.network_frame();
    dead.carts[0].alive = false;
    movement.observe(&dead, 0.125, 0);
    sim.step();
    let respawn = sim.network_frame();
    movement.observe(&respawn, 0.140625, 0);
    assert_eq!(
        movement.local_position(),
        Some(respawn.carts[0].position),
        "respawn starts from authority with no previous-life suffix"
    );
}

#[test]
fn local_rendering_interpolates_between_actual_fixed_steps() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let start = sim.network_frame();
    movement.observe(&start, 0.0, 0);
    movement.advance(1.0 / 64.0, Default::default());
    let current = movement.local_position().unwrap();
    let previous = movement.local_pose().unwrap().0;
    movement.advance(1.0 / 128.0, Default::default());
    let middle = movement.local_pose().unwrap().0;
    for i in 0..3 {
        assert!((middle[i] - (previous[i] + current[i]) * 0.5).abs() < 1e-5);
    }
    assert_ne!(middle, previous);
    assert_ne!(middle, current);
}

#[test]
fn small_corrections_fix_simulation_immediately_but_decay_visual_residual_by_time() {
    fn corrected() -> Movement {
        let mut sim = Simulation::new(Default::default()).unwrap();
        let mut movement = Movement::new(sim.network_edges());
        movement.observe(&sim.network_frame(), 0.0, 0);
        for _ in 0..8 {
            movement.advance(1.0 / 64.0, Default::default());
        }
        for _ in 0..4 {
            sim.step();
        }
        let before = movement.local_pose().unwrap().0;
        let mut correction = sim.network_frame();
        correction.motion.as_mut().unwrap().rails[0].progress += 0.05;
        movement.observe(&correction, 0.125, 0);
        assert!(movement.local_position().unwrap()[0] > before[0]);
        let after = movement.local_pose().unwrap().0;
        for axis in 0..3 {
            assert!(
                (after[axis] - before[axis]).abs() < 1e-5,
                "small correction must not snap the rendered cart"
            );
        }
        movement
    }
    let mut sixty = corrected();
    let mut one_twenty = corrected();
    for _ in 0..6 {
        sixty.advance(1.0 / 60.0, Default::default());
    }
    for _ in 0..12 {
        one_twenty.advance(1.0 / 120.0, Default::default());
    }
    let a = sixty.local_pose().unwrap().0;
    let b = one_twenty.local_pose().unwrap().0;
    for axis in 0..3 {
        assert!(
            (a[axis] - b[axis]).abs() < 1e-5,
            "decay must not depend on render partition"
        );
    }
}

#[test]
fn a_regressing_simulation_clock_cannot_replay_an_incomplete_suffix() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..8 {
        movement.advance(1.0 / 64.0, Default::default());
    }
    for _ in 0..4 {
        sim.step();
    }
    let fresh = sim.network_frame();
    movement.observe(&fresh, 0.1, 0);
    let before = movement.local_position();
    let mut regressed = fresh;
    regressed.tick += 1;
    regressed.simulation_tick = 2;
    regressed.motion.as_mut().unwrap().rails[0].progress -= 0.01;
    movement.observe(&regressed, 0.2, 0);
    assert_eq!(
        movement.local_position(),
        before,
        "publication freshness cannot validate a regressing simulation clock"
    );
}

#[test]
fn maximum_authoritative_tick_disables_speculation_without_panicking() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let mut frame = sim.network_frame();
    frame.simulation_tick = u64::MAX;
    movement.observe(&frame, 0.0, 0);
    movement.advance(1.0 / 64.0, Default::default());
    assert!(movement.local_position().is_none());
}

#[test]
fn a_reversal_correction_discards_pre_collision_speculation() {
    use ryggattack::network::RailPath;
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..8 {
        movement.advance(1.0 / 64.0, Default::default());
    }
    for _ in 0..4 {
        sim.step();
    }
    let mut reversed = sim.network_frame();
    let rail = &mut reversed.motion.as_mut().unwrap().rails[0];
    let RailPath::Edge { from, to } = rail.path else {
        panic!("edge fixture");
    };
    rail.path = RailPath::Edge { from: to, to: from };
    rail.progress = 1.0 - rail.progress;
    movement.observe(&reversed, 0.1, 0);
    assert_eq!(
        movement.local_position(),
        Some(reversed.carts[0].position),
        "reversal snaps to authority without old suffix"
    );
    assert_eq!(movement.local_pose().unwrap().0, reversed.carts[0].position);
}

#[test]
fn a_stall_clears_remote_interpolation_until_fresh_authority() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for tick in 1..=8 {
        sim.step();
        if tick % 4 == 0 {
            movement.observe(&sim.network_frame(), tick as f64 / 64.0, 0);
        }
    }
    movement.advance(10.0, Default::default());
    assert!(
        movement.remote_pose(1, 0.15625).is_none(),
        "caller must fall back to current authority after a stall"
    );
}

#[test]
fn remote_motion_on_an_absent_track_falls_back_to_cart_authority() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let initial = sim.network_frame();
    let path = initial.motion.as_ref().unwrap().rails[1].path;
    let ryggattack::network::RailPath::Edge { from, to } = path else {
        panic!("edge fixture");
    };
    let edges = sim
        .network_edges()
        .into_iter()
        .filter(|&(a, b)| (a, b) != (from, to) && (a, b) != (to, from))
        .collect();
    let mut movement = Movement::new(edges);
    movement.observe(&initial, 0.0, 0);
    for tick in 1..=8 {
        sim.step();
        if tick % 4 == 0 {
            movement.observe(&sim.network_frame(), tick as f64 / 64.0, 0);
        }
    }
    assert_eq!(
        movement.remote_pose(1, 0.15625).unwrap().0,
        sim.snapshot().carts[1].position
    );
}

#[test]
fn movement_snapshot_exposes_authoritative_directed_rail_state() {
    let mut sim = Simulation::new(SimulationConfig::default()).unwrap();
    sim.step();
    let value = serde_json::to_value(sim.network_frame()).unwrap();
    assert!(
        value["motion"].is_object(),
        "prediction needs exact rail state, not an inverse world-position guess"
    );
    assert!(
        value["motion"]["rails"]
            .as_array()
            .is_some_and(|rails| rails.len() == 4)
    );
}
