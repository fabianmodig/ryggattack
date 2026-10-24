//! Windowless execution of the *same* gameplay systems used by offline play.
//! This is an in-process simulation harness, not an online server or protocol.
pub use crate::match_settings::Arena;
use crate::{
    combat::Projectile,
    game::{AppState, Round},
    input::{ControlIntent, SeatInputs},
    match_settings::MatchSettings,
    players::{Player, Roster},
    powerups::{Pickup, Powerups},
    scenery::Scenery,
    settings::VideoSettings,
    simulation::{PresentationPlugin, SimulationPlugin},
    tracks::RailMap,
};
use bevy::{ecs::system::RunSystemOnce, prelude::*};

pub use crate::simulation::{TICK_DURATION, TICK_HZ};

#[derive(Clone, Debug)]
pub struct SimulationConfig {
    pub seed: u64,
    pub human_seats: [bool; 4],
    /// Bots fill the lowest available seats, just as in the offline lobby.
    pub bots: usize,
    pub wins_target: u32,
    pub arena: Arena,
}
impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            human_seats: [true, false, false, false],
            bots: 3,
            wins_target: 3,
            arena: Arena::Forest,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CartSnapshot {
    pub id: usize,
    pub active: bool,
    pub alive: bool,
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub shot_cooldown: f32,
    pub speed_seconds: f32,
    pub all_hits_seconds: f32,
}

/// Diagnostic view, deliberately NOT a complete replication or save format.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub carts: Vec<CartSnapshot>,
    pub wins: [u32; 4],
    pub winner: Option<usize>,
    pub round_finished: bool,
    pub transition: f32,
    pub projectiles: usize,
    pub pickups: usize,
    pub scenery: usize,
    pub until_drop: f32,
}

pub struct Simulation {
    pub(crate) app: App,
    tick: u64,
    network_ids: std::collections::HashMap<Entity, u64>,
    next_network_id: u64,
}
impl Simulation {
    pub fn new(config: SimulationConfig) -> Result<Self, &'static str> {
        if config.bots > 3 {
            return Err("bot count must be 0..=3 (and is capped by free seats)");
        }
        if !matches!(config.wins_target, 1 | 3 | 5 | 7) {
            return Err("win target must be 1, 3, 5, or 7");
        }
        if !config.human_seats.iter().any(|human| *human) {
            return Err("at least one externally controlled seat is required");
        }
        let mut app = App::new();
        app.insert_resource(State::new(AppState::Playing))
            .init_resource::<Time>()
            .init_resource::<Round>()
            .init_resource::<Roster>()
            .insert_resource(Powerups::from_seed(config.seed.rotate_left(29)))
            .init_resource::<crate::ui::PauseState>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<VideoSettings>()
            .insert_resource(MatchSettings {
                bots: config.bots,
                wins_target: config.wins_target,
                arena: config.arena,
            })
            .insert_resource(RailMap::from_seed(config.seed))
            .add_plugins((SimulationPlugin, PresentationPlugin))
            .insert_resource(SeatInputs {
                external: config.human_seats,
                occupied: Some(
                    Roster::default().active_seats_with_external(config.bots, config.human_seats),
                ),
                ..default()
            })
            .add_systems(Startup, setup);
        // App::new has no TimePlugin, render, window, audio, or device plugins.
        // Startup only constructs CPU-side state; every tick is explicit below.
        app.update();
        app.world_mut()
            .run_system_once(crate::game::start_new_game)
            .expect("shared round reset is initialized");
        Ok(Self {
            app,
            tick: 0,
            network_ids: Default::default(),
            next_network_id: 1,
        })
    }

    /// Replace held intent. Neutral input does not hand this seat to a bot.
    /// Callers, not display names, must eventually be authenticated by transport.
    pub fn submit(&mut self, seat: usize, intent: ControlIntent) -> Result<(), &'static str> {
        if !intent.horizontal.is_finite()
            || !intent.vertical.is_finite()
            || intent.horizontal.abs() > 1.0
            || intent.vertical.abs() > 1.0
        {
            return Err("intent axes must be finite and within -1..=1");
        }
        let mut inputs = self.app.world_mut().resource_mut::<SeatInputs>();
        if !inputs.external.get(seat).copied().unwrap_or(false) {
            return Err("seat is not externally controlled");
        }
        inputs.intents[seat] = intent;
        Ok(())
    }

    /// Change only the controller, never character identity, lifetime or buffs.
    pub fn set_external(&mut self, seat: usize, external: bool) -> Result<(), &'static str> {
        let world = self.app.world_mut();
        {
            let mut inputs = world.resource_mut::<SeatInputs>();
            if !inputs
                .occupied
                .is_some_and(|a| a.get(seat).copied().unwrap_or(false))
            {
                return Err("inactive seat");
            }
            inputs.external[seat] = external;
            inputs.intents[seat] = ControlIntent::default();
            inputs.local_fire_pending[seat] = false;
        }
        for (mut p, mut rail) in world
            .query::<(&mut Player, &mut crate::tracks::RailFollower)>()
            .iter_mut(world)
        {
            if p.id == seat {
                p.wants_to_fire = false;
                rail.requested_turn = Vec3::ZERO;
            }
        }
        Ok(())
    }

    /// Exactly one gameplay tick, independent of wall time and render cadence.
    pub fn step(&mut self) {
        let world = self.app.world_mut();
        let generic = {
            let mut fixed = world.resource_mut::<Time<Fixed>>();
            fixed.advance_by(TICK_DURATION);
            fixed.as_generic()
        };
        // Mirror Bevy's fixed-loop clock view for both Res<Time> and
        // Res<Time<Fixed>> consumers, without installing a wall clock runner.
        *world.resource_mut::<Time>() = generic;
        world.run_schedule(FixedUpdate);
        // Presentation lifetimes still need cleanup while their construction
        // remains coupled to gameplay. No render schedule or GPU is installed.
        self.app.world_mut().run_schedule(Update);
        self.tick += 1;
    }

    /// Explicit topology: clients must never regenerate random map state.
    pub fn network_edges(&self) -> Vec<(usize, usize)> {
        self.app.world().resource::<RailMap>().edges.clone()
    }

    /// Presentation replication with stable monotonic IDs, not ECS handles.
    /// Props use their real collider bounds; transient effects are visual only.
    pub fn network_frame(&mut self) -> crate::network::Frame {
        use crate::network::{Body, Frame};
        let snap = self.snapshot();
        let world = self.app.world_mut();
        let mut live = std::collections::HashSet::new();
        let mut bodies = Vec::new();
        for (entity, transform, projectile, pickup, scenery, fireball) in world
            .query::<(
                Entity,
                &Transform,
                Option<&Projectile>,
                Option<&Pickup>,
                Option<&Scenery>,
                Option<&crate::explosions::Fireball>,
            )>()
            .iter(world)
        {
            let kind = if projectile.is_some() {
                0
            } else if let Some(pickup) = pickup {
                if pickup.0 == crate::powerups::Kind::Speed {
                    1
                } else {
                    2
                }
            } else if scenery.is_some() {
                3
            } else if fireball.is_some() {
                4
            } else {
                continue;
            };
            live.insert(entity);
            let id = *self.network_ids.entry(entity).or_insert_with(|| {
                let id = self.next_network_id;
                self.next_network_id += 1;
                id
            });
            let mut position = transform.translation;
            let scale = if let Some(prop) = scenery {
                position.y = prop.height * 0.5;
                [prop.hit_radius * 2.0, prop.height, prop.hit_radius * 2.0]
            } else {
                transform.scale.to_array()
            };
            bodies.push(Body {
                id,
                position: position.to_array(),
                rotation: transform.rotation.to_array(),
                scale,
                kind,
            });
        }
        self.network_ids.retain(|entity, _| live.contains(entity));
        bodies.sort_by_key(|body| body.id);
        Frame {
            tick: snap.tick,
            simulation_tick: snap.tick,
            ack: [0; 4],
            names: Default::default(),
            carts: snap.carts,
            wins: snap.wins,
            winner: snap.winner,
            round_finished: snap.round_finished,
            transition: snap.transition,
            until_drop: snap.until_drop,
            bodies,
        }
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let world = self.app.world_mut();
        let buffs = world.resource::<Powerups>().buffs;
        let mut carts: Vec<_> = world
            .query::<(&Player, &Transform)>()
            .iter(world)
            .map(|(p, t)| CartSnapshot {
                id: p.id,
                active: p.active,
                alive: p.alive,
                position: t.translation.to_array(),
                forward: t.forward().as_vec3().to_array(),
                shot_cooldown: p.shot_cooldown,
                speed_seconds: buffs[p.id].speed,
                all_hits_seconds: buffs[p.id].all_hits,
            })
            .collect();
        carts.sort_by_key(|p| p.id);
        let projectiles = world.query::<&Projectile>().iter(world).count();
        let pickups = world.query::<&Pickup>().iter(world).count();
        let scenery = world.query::<&Scenery>().iter(world).count();
        let round = world.resource::<Round>();
        Snapshot {
            tick: self.tick,
            carts,
            wins: round.wins,
            winner: round.winner,
            round_finished: round.finished,
            transition: round.transition,
            projectiles,
            pickups,
            scenery,
            until_drop: world.resource::<Powerups>().until_drop,
        }
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map: Res<RailMap>,
    settings: Res<MatchSettings>,
) {
    crate::scene::setup_gameplay(
        &mut commands,
        &mut meshes,
        &mut materials,
        &map,
        settings.arena,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_step_advances_both_fixed_and_generic_game_clocks() {
        let mut simulation = Simulation::new(SimulationConfig::default()).unwrap();
        simulation.step();
        assert_eq!(
            simulation.app.world().resource::<Time<Fixed>>().elapsed(),
            TICK_DURATION
        );
        assert_eq!(
            simulation.app.world().resource::<Time>().elapsed(),
            TICK_DURATION
        );
        assert_eq!(
            simulation.app.world().resource::<Time<Fixed>>().delta(),
            TICK_DURATION
        );
    }

    #[test]
    fn control_switch_preserves_character_buffs_and_active_roster() {
        let mut sim = Simulation::new(SimulationConfig {
            human_seats: [true, true, false, false],
            bots: 0,
            ..Default::default()
        })
        .unwrap();
        sim.app.world_mut().resource_mut::<Powerups>().buffs[0].speed = 5.;
        let before = sim.snapshot();
        sim.set_external(0, false).unwrap();
        assert_eq!(
            sim.snapshot(),
            before,
            "role switch must not respawn or advance clocks"
        );
        assert!(sim.submit(0, ControlIntent::default()).is_err());
        sim.step();
        let world = sim.app.world_mut();
        assert!(
            world
                .query::<(&Player, &crate::tracks::RailFollower)>()
                .iter(world)
                .any(|(p, r)| p.id == 0 && r.requested_turn != Vec3::ZERO),
            "real bot AI chooses a target turn"
        );
        sim.set_external(0, true).unwrap();
        sim.submit(0, ControlIntent::default()).unwrap();
        sim.app
            .world_mut()
            .run_system_once(crate::game::start_new_game)
            .unwrap();
        assert_eq!(
            sim.snapshot()
                .carts
                .iter()
                .map(|c| c.active)
                .collect::<Vec<_>>(),
            vec![true, true, false, false]
        );
    }
    #[test]
    fn automatic_round_restoration_reuses_external_seats_and_existing_gameplay_reset() {
        let mut simulation = Simulation::new(SimulationConfig {
            human_seats: [true, true, false, false],
            bots: 0,
            ..default()
        })
        .unwrap();
        let original_scenery = simulation.snapshot().scenery;
        let world = simulation.app.world_mut();
        let prop = world
            .query_filtered::<Entity, With<Scenery>>()
            .iter(world)
            .next()
            .unwrap();
        world.despawn(prop);
        let transient = world.spawn(crate::scenery::Transient).id();
        for mut player in world.query::<&mut Player>().iter_mut(world) {
            if player.id == 1 {
                player.alive = false;
            }
        }
        world.resource_mut::<Powerups>().buffs[0].grant(crate::powerups::Kind::Speed);
        simulation.step();
        assert!(simulation.snapshot().round_finished);
        assert_eq!(simulation.snapshot().wins, [1, 0, 0, 0]);
        for _ in 0..96 {
            simulation.step();
        }
        let snapshot = simulation.snapshot();
        assert!(!snapshot.round_finished);
        assert_eq!(snapshot.wins, [1, 0, 0, 0]);
        assert!(snapshot.carts[0].alive && snapshot.carts[1].alive);
        assert_eq!(snapshot.carts[0].speed_seconds, 0.0);
        assert_eq!(snapshot.scenery, original_scenery);
        assert_eq!(snapshot.until_drop, 30.0);
        assert!(simulation.app.world().get_entity(transient).is_err());
        assert!(simulation.submit(1, ControlIntent::default()).is_ok());
    }

    #[test]
    fn volcano_uses_the_existing_themed_scenery_colliders() {
        let mut simulation = Simulation::new(SimulationConfig {
            arena: Arena::Volcano,
            ..default()
        })
        .unwrap();
        let world = simulation.app.world_mut();
        assert_eq!(
            world.resource::<crate::scenery::Forest>().arena,
            Arena::Volcano
        );
        assert!(
            world
                .query::<&Scenery>()
                .iter(world)
                .all(|prop| prop.height <= 0.82501)
        );
    }
}
