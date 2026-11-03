//! Device-independent held intent. No positions, hits, or results cross this boundary.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlIntent {
    /// Screen-right and screen-up axes. Adapters normalize before use.
    pub horizontal: f32,
    pub vertical: f32,
    pub fire: bool,
}

#[derive(Resource, Default)]
pub(crate) struct TouchInput {
    pub(crate) intent: ControlIntent,
    pub(crate) reset: bool,
}

/// Sampled once per render frame; tap fire is latched by the existing adapters.
/// An epoch change invalidates fire pending before cancellation, even between ticks.
pub(crate) fn sample_touch(mut touch: ResMut<TouchInput>, mut epoch: Local<u32>) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        #[derive(serde::Deserialize)]
        struct Sample {
            horizontal: f32,
            vertical: f32,
            fire: bool,
            epoch: u32,
        }
        let global = js_sys::global();
        let sample = js_sys::Reflect::get(&global, &"ryggattackSampleTouch".into())
            .ok()
            .and_then(|v| v.dyn_into::<js_sys::Function>().ok())
            .and_then(|f| f.call0(&global).ok())
            .and_then(|v| v.as_string())
            .and_then(|v| serde_json::from_str::<Sample>(&v).ok());
        if let Some(sample) = sample {
            touch.reset = *epoch != sample.epoch;
            *epoch = sample.epoch;
            touch.intent = ControlIntent {
                horizontal: sample.horizontal,
                vertical: sample.vertical,
                fire: sample.fire,
            };
        } else {
            touch.intent = default();
            touch.reset = true;
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (&mut touch, &mut epoch);
}

pub(crate) fn merge_touch(mut device: ControlIntent, touch: ControlIntent) -> ControlIntent {
    // Active desktop steering takes precedence; a swipe is a retained fallback.
    if device.horizontal == 0.0 && device.vertical == 0.0 {
        device.horizontal = touch.horizontal;
        device.vertical = touch.vertical;
    }
    device.fire |= touch.fire;
    device
}

#[derive(Resource, Default)]
pub(crate) struct SeatInputs {
    pub(crate) intents: [ControlIntent; 4],
    /// Non-device controlled seats (for the headless caller). Local seats are
    /// still selected by the offline roster; an empty intent is neutral, not a bot.
    pub(crate) external: [bool; 4],
    pub(crate) occupied: Option<[bool; 4]>,
    /// The local frame adapter retains a sampled press until one tick observes it.
    /// External submitters still replace held intent; no remote action queue is implied.
    pub(crate) local_fire_pending: [bool; 4],
}

pub(crate) fn apply_inputs(
    mut inputs: ResMut<SeatInputs>,
    round: Res<crate::game::Round>,
    mut players: Query<(
        &mut crate::players::Player,
        &mut crate::tracks::RailFollower,
    )>,
) {
    for (mut player, mut rail) in &mut players {
        let local_fire = std::mem::take(&mut inputs.local_fire_pending[player.id]);
        if !player.active || !player.alive || round.finished {
            player.wants_to_fire = false;
            rail.requested_turn = Vec3::ZERO;
        } else if player.input.is_some() || inputs.external[player.id] {
            let intent = inputs.intents[player.id];
            rail.requested_turn =
                Vec3::new(intent.horizontal, 0.0, -intent.vertical).normalize_or_zero();
            player.wants_to_fire = intent.fire || local_fire;
        }
    }
}

#[cfg(test)]
mod touch_tests {
    use super::*;
    #[test]
    fn desktop_turns_take_precedence_but_touch_fire_joins_the_existing_intent() {
        let touch = ControlIntent {
            horizontal: 1.0,
            vertical: 0.0,
            fire: true,
        };
        assert_eq!(merge_touch(default(), touch), touch);
        assert_eq!(
            merge_touch(
                ControlIntent {
                    vertical: 1.0,
                    ..default()
                },
                touch
            ),
            ControlIntent {
                horizontal: 0.0,
                vertical: 1.0,
                fire: true
            }
        );
    }
    #[test]
    fn touch_taps_use_real_fixed_simulation_cooldown_without_queued_or_repeated_shots() {
        use crate::players::{InputSource, Player, Roster, human_input};
        let mut sim = crate::headless::Simulation::new(crate::headless::SimulationConfig {
            bots: 0,
            ..default()
        })
        .unwrap();
        let world = sim.app.world_mut();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<TouchInput>();
        world.resource_mut::<Roster>().join(InputSource::Wasd);
        for mut p in world.query::<&mut Player>().iter_mut(world) {
            if p.id == 0 {
                p.input = Some(InputSource::Wasd);
            }
        }
        sim.app.add_systems(PreUpdate, human_input);
        // Two render samples before a tick: a touch release must not lose the tap.
        sim.app.world_mut().resource_mut::<TouchInput>().intent.fire = true;
        sim.app.update();
        sim.app.world_mut().resource_mut::<TouchInput>().intent.fire = false;
        sim.app.update();
        sim.step();
        assert!(sim.snapshot().carts[0].shot_cooldown > 0.0);
        let first = sim.snapshot().projectiles;
        assert_eq!(first, 1);
        sim.step();
        assert_eq!(sim.snapshot().projectiles, first);
        sim.app.world_mut().resource_mut::<TouchInput>().intent.fire = true;
        sim.app.update();
        sim.step(); // a second tap during cooldown is not queued
        sim.app.world_mut().resource_mut::<TouchInput>().intent.fire = false;
        sim.app.update();
        assert_eq!(sim.snapshot().projectiles, first);
        for _ in 0..70 {
            sim.step();
        }
        assert_eq!(sim.snapshot().carts[0].shot_cooldown, 0.0);
        sim.app.world_mut().resource_mut::<TouchInput>().intent.fire = true;
        sim.app.update();
        sim.step();
        assert!(sim.snapshot().carts[0].shot_cooldown > 0.0);
    }
}
