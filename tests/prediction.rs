use ryggattack::headless::{Simulation, SimulationConfig};
use ryggattack::{input::ControlIntent, prediction::Movement};

#[test]
fn prediction_uses_shared_fixed_travel_before_new_authority() {
    let mut sim = Simulation::new(SimulationConfig {
        bots: 0,
        human_seats: [true, true, false, false],
        ..Default::default()
    }).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let initial = sim.network_frame();
    movement.observe(&initial, 0.0, 0);
    let intent = ControlIntent { horizontal: 1.0, vertical: 1.0, fire: true };
    for _ in 0..120 {
        movement.advance(1.0/64.0, intent);
        sim.submit(0, intent).unwrap();
        sim.step();
        // Refresh authority periodically but still exercise actual outstanding steps.
        let predicted = movement.local_position().unwrap();
        let authoritative = sim.snapshot().carts[0].position;
        for axis in 0..3 {
            assert!((predicted[axis]-authoritative[axis]).abs()<1e-4);
        }
        let tick = sim.snapshot().tick;
        if tick % 8 == 0 {
            movement.observe(&sim.network_frame(), tick as f64/64.0, 0);
        }
    }
}

#[test]
fn reconciliation_replays_actual_steps_not_acknowledged_messages() {
    let mut sim = Simulation::new(SimulationConfig { bots: 0, human_seats: [true, true, false, false], ..Default::default() }).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    let right = ControlIntent { horizontal: 1.0, ..Default::default() };
    let up = ControlIntent { vertical: 1.0, ..Default::default() };
    for _ in 0..4 { movement.advance(1.0/64.0, right); sim.submit(0,right).unwrap(); sim.step(); }
    let mut acknowledged = sim.network_frame();
    // A held message has already been acknowledged, but future actual steps
    // using that same held intent must not be discarded on a delivery watermark.
    acknowledged.ack[0] = 999;
    for _ in 0..4 { movement.advance(1.0/64.0, up); sim.submit(0,up).unwrap(); sim.step(); }
    let expected = sim.snapshot().carts[0].position;
    movement.observe(&acknowledged, 0.125, 0);
    assert_eq!(movement.local_position().unwrap(), expected, "restore tick 4 then replay exactly steps 5..8");
}

#[test]
fn a_long_render_stall_discards_speculation_instead_of_catching_up() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    movement.advance(10.0, Default::default());
    assert!(movement.local_position().is_none(), "fallback to authority after a long stall");
}

#[test]
fn prediction_horizon_is_bounded_even_with_many_small_render_frames() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    movement.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..240 { movement.advance(1.0/240.0, Default::default()); }
    assert!(movement.local_position().is_none(), "a small delta cannot evade the half-second horizon");
}

#[test]
fn stale_or_duplicate_frames_do_not_rewind_prediction() {
    let mut sim = Simulation::new(SimulationConfig { bots: 0, ..Default::default() }).unwrap();
    let mut movement = Movement::new(sim.network_edges());
    let old = sim.network_frame();
    movement.observe(&old, 0.0, 0);
    for _ in 0..4 { sim.step(); movement.advance(1.0/64.0, Default::default()); }
    let fresh = sim.network_frame();
    movement.observe(&fresh, 0.0625, 0);
    for _ in 0..4 { movement.advance(1.0/64.0, Default::default()); }
    let before = movement.local_position();
    movement.observe(&fresh, 0.125, 0);
    movement.observe(&old, 0.126, 0);
    assert_eq!(movement.local_position(), before);
}

#[test]
fn movement_snapshot_exposes_authoritative_directed_rail_state() {
    let mut sim = Simulation::new(SimulationConfig::default()).unwrap();
    sim.step();
    let value = serde_json::to_value(sim.network_frame()).unwrap();
    assert!(value["motion"].is_object(), "prediction needs exact rail state, not an inverse world-position guess");
    assert!(value["motion"]["rails"].as_array().is_some_and(|rails| rails.len() == 4));
}
