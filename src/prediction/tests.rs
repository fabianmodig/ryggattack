use super::*;
use crate::headless::{Simulation, SimulationConfig};
use crate::players::Player;

#[test]
fn shared_speed_expiry_and_contact_corrections_do_not_predict_outcomes() {
    for front in [0, 1] {
        let mut sim = Simulation::new(SimulationConfig {
            bots: 0,
            human_seats: [true, true, false, false],
            ..Default::default()
        })
        .unwrap();
        let path = RailPath::Edge { from: 0, to: 1 };
        let world = sim.app.world_mut();
        world.resource_mut::<crate::powerups::Powerups>().buffs[front].speed = 1.0 / 64.0;
        for (player, mut rail, mut pose) in world
            .query::<(&Player, &mut RailFollower, &mut Transform)>()
            .iter_mut(world)
        {
            if !player.active {
                continue;
            }
            rail.path = path;
            rail.progress = if player.id == front { 0.5 } else { 0.0 };

            let (position, direction) = track_pose(path, rail.progress);
            *pose = Transform::from_translation(position).looking_to(direction, Vec3::Y);
        }
        let mut model = Movement::new(sim.network_edges());
        model.observe(&sim.network_frame(), 0.0, front);
        model.advance(
            1.0 / 64.0,
            ControlIntent {
                fire: true,
                ..Default::default()
            },
        );
        let predicted = model.local_position().unwrap();
        sim.step();
        let frame = sim.network_frame();
        let actual = frame.carts[front].position;
        assert!(
            (Vec3::from_array(actual).distance(Vec3::from_array(predicted)) - 0.4).abs() < 1e-4,
            "only authority performs rear contact"
        );
        assert_eq!(frame.carts[front].speed_seconds, 0.0);
        model.observe(&frame, 1.0 / 64.0, front);
        assert_eq!(model.local_position(), Some(actual));
        let before = sim.snapshot();
        model.advance(
            1.0 / 64.0,
            ControlIntent {
                fire: true,
                ..Default::default()
            },
        );
        assert_eq!(
            sim.snapshot(),
            before,
            "speculation cannot spawn projectiles, change scores or grant pickups"
        );
        sim.step();
        let actual = sim.snapshot().carts[front].position;
        assert!(
            Vec3::from_array(actual).distance(Vec3::from_array(model.local_position().unwrap()))
                < 1e-4,
            "shared speed expiry uses the same fixed-tick order"
        );
    }
}

#[test]
fn a_missing_history_step_falls_back_instead_of_silently_shortening_replay() {
    let mut sim = Simulation::new(Default::default()).unwrap();
    let mut model = Movement::new(sim.network_edges());
    model.observe(&sim.network_frame(), 0.0, 0);
    for _ in 0..8 {
        model.advance(1.0 / 64.0, Default::default());
    }
    model.history.remove(5);
    for _ in 0..4 {
        sim.step();
    }
    let frame = sim.network_frame();
    model.observe(&frame, 0.1, 0);
    assert_eq!(model.local_position(), Some(frame.carts[0].position));
    assert!(model.history.is_empty());
    assert_eq!(model.head, frame.simulation_tick);
}
