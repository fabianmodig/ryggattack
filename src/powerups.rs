//! Rail drops and temporary per-seat effects, measured in active round time.
use crate::tracks::SimpleRng;
use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct Powerups {
    pub(crate) until_drop: f32,
    pub(crate) buffs: [Buffs; 4],
    previous: [Option<Vec3>; 4],
    rng: SimpleRng,
}
impl Default for Powerups {
    fn default() -> Self {
        let duration = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default();
        let seed = duration.as_secs() ^ (duration.subsec_nanos() as u64).rotate_left(32);
        Self {
            until_drop: 30.0,
            buffs: [Buffs::default(); 4],
            previous: [None; 4],
            rng: SimpleRng::new(seed),
        }
    }
}
impl Powerups {
    pub(crate) fn reset(&mut self) {
        self.until_drop = 30.0;
        self.buffs = [Buffs::default(); 4];
        self.previous = [None; 4];
    }
    fn advance(&mut self, dt: f32) -> usize {
        self.until_drop -= dt;
        let mut count = 0;
        while self.until_drop <= 0.0 {
            count += 1;
            self.until_drop += self.rng.range(5.0, 20.0);
        }
        count
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Speed,
    AllHits,
}
#[derive(Default, Clone, Copy)]
pub(crate) struct Buffs {
    pub(crate) speed: f32,
    pub(crate) all_hits: f32,
}
impl Buffs {
    pub(crate) fn grant(&mut self, kind: Kind) {
        match kind {
            Kind::Speed => self.speed = 10.0,
            Kind::AllHits => self.all_hits = 10.0,
        }
    }
    fn tick(&mut self, dt: f32) {
        self.speed = (self.speed - dt).max(0.0);
        self.all_hits = (self.all_hits - dt).max(0.0);
    }
    pub(crate) fn multiplier(self) -> f32 {
        if self.speed > 0.0 { 1.25 } else { 1.0 }
    }
}
#[derive(Component)]
pub(crate) struct Pickup(pub(crate) Kind);
#[derive(Default)]
pub(crate) struct PickupAssets {
    meshes: Option<[Handle<Mesh>; 2]>,
    materials: Option<[Handle<StandardMaterial>; 2]>,
}

/// Called only during a live, unpaused round, before movement.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick_and_drop(
    mut commands: Commands,
    time: Res<Time>,
    map: Res<crate::tracks::RailMap>,
    mut powerups: ResMut<Powerups>,
    players: Query<(&crate::players::Player, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<PickupAssets>,
) {
    for buffs in &mut powerups.buffs {
        buffs.tick(time.delta_secs());
    }
    powerups.previous = [None; 4];
    for (player, transform) in &players {
        if player.active && player.alive {
            powerups.previous[player.id] = Some(transform.translation);
        }
    }
    let count = powerups.advance(time.delta_secs());
    if count == 0 {
        return;
    }
    let pickup_meshes = assets
        .meshes
        .get_or_insert_with(|| {
            [
                meshes.add(Cuboid::new(0.65, 0.65, 0.65)),
                meshes.add(Torus::new(0.28, 0.5)),
            ]
        })
        .clone();
    let pickup_materials = assets
        .materials
        .get_or_insert_with(|| {
            [Color::srgb(1.0, 0.85, 0.05), Color::srgb(1.0, 0.12, 0.75)].map(|color| {
                materials.add(StandardMaterial {
                    base_color: color,
                    unlit: true,
                    ..default()
                })
            })
        })
        .clone();
    for _ in 0..count {
        let index = powerups.rng.index(2);
        let kind = if index == 0 {
            Kind::Speed
        } else {
            Kind::AllHits
        };
        let position = map.random_drop_position(&mut powerups.rng).with_y(1.15);
        let rotation = if index == 0 {
            Quat::from_euler(EulerRot::XYZ, 0.6, 0.0, 0.7)
        } else {
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
        };
        commands.spawn((
            Pickup(kind),
            crate::scenery::Transient,
            Mesh3d(pickup_meshes[index].clone()),
            MeshMaterial3d(pickup_materials[index].clone()),
            Transform::from_translation(position).with_rotation(rotation),
        ));
    }
}

/// Sweep the cart's movement in the ground plane, so a slow frame cannot skip a drop.
pub(crate) fn collect(
    mut commands: Commands,
    mut powerups: ResMut<Powerups>,
    players: Query<(&crate::players::Player, &Transform), Without<Pickup>>,
    pickups: Query<(Entity, &Pickup, &Transform)>,
) {
    for (entity, pickup, transform) in &pickups {
        // Stable seat order resolves simultaneous claims identically for humans and bots.
        let winner = players
            .iter()
            .filter(|(p, _)| p.active && p.alive)
            .filter(|(p, t)| {
                let end = t.translation.xz();
                let start = powerups.previous[p.id].unwrap_or(t.translation).xz();
                let delta = end - start;
                let fraction = if delta.length_squared() > 0.0 {
                    ((transform.translation.xz() - start).dot(delta) / delta.length_squared())
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                transform
                    .translation
                    .xz()
                    .distance(start + delta * fraction)
                    <= 0.8
            })
            .map(|(p, _)| p.id)
            .min();
        if let Some(id) = winner {
            powerups.buffs[id].grant(pickup.0);
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::Round, players::Player, tracks::RailMap};
    #[test]
    fn round_reset_keeps_random_sequence_instead_of_replaying_seed_one() {
        let mut drops = Powerups {
            rng: SimpleRng::new(42),
            ..default()
        };
        let mut expected = SimpleRng::new(42);
        assert_eq!(drops.advance(30.0), 1);
        assert_eq!(drops.until_drop, expected.range(5.0, 20.0));
        drops.buffs[0].grant(Kind::AllHits);
        drops.previous[0] = Some(Vec3::X);
        drops.reset();
        assert_eq!(drops.until_drop, 30.0);
        assert_eq!(drops.buffs[0].all_hits, 0.0);
        assert_eq!(drops.previous, [None; 4]);
        assert_eq!(drops.advance(30.0), 1);
        assert_eq!(drops.until_drop, expected.range(5.0, 20.0));
    }
    #[test]
    fn finished_round_and_pause_freeze_drops_and_buffs() {
        let mut app = app();
        app.init_resource::<crate::ui::PauseState>();
        app.world_mut().resource_mut::<Powerups>().buffs[0].grant(Kind::Speed);
        app.world_mut().resource_mut::<Round>().finished = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(60));
        app.update();
        assert_eq!(app.world().resource::<Powerups>().until_drop, 30.0);
        assert_eq!(app.world().resource::<Powerups>().buffs[0].speed, 10.0);
        // Pause is exercised through the same predicate that gates the production chain.
        app.world_mut().resource_mut::<Round>().finished = false;
        app.add_systems(Update, crate::ui::toggle_pause_dialog.before(tick_and_drop));
        app.insert_resource(crate::ui::MenuInput {
            cancel: true,
            ..default()
        });
        // Gate this test's schedule just as main gates its gameplay chain.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::ZERO);
        app.update();
        app.world_mut()
            .resource_mut::<crate::ui::MenuInput>()
            .cancel = false;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(60));
        app.update();
        assert_eq!(app.world().resource::<Powerups>().until_drop, 30.0);
        assert_eq!(app.world().resource::<Powerups>().buffs[0].speed, 10.0);
    }
    #[test]
    fn swept_pickup_has_one_stable_winner_and_ignores_dead_or_empty_seats() {
        let mut app = App::new();
        app.init_resource::<Powerups>();
        for (id, active, alive) in [
            (3, true, true),
            (2, true, true),
            (1, false, true),
            (0, true, false),
        ] {
            app.world_mut().spawn((
                Player {
                    id,
                    active,
                    alive,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: None,
                },
                Transform::from_xyz(2.0, 0.62, 0.0),
            ));
            app.world_mut().resource_mut::<Powerups>().previous[id] =
                Some(Vec3::new(-2.0, 0.62, 0.0));
        }
        let drop = app
            .world_mut()
            .spawn((Pickup(Kind::AllHits), Transform::from_xyz(0.0, 1.15, 0.0)))
            .id();
        app.add_systems(Update, collect);
        app.update();
        assert!(app.world().get_entity(drop).is_err());
        let buffs = app.world().resource::<Powerups>().buffs;
        assert_eq!(buffs.map(|b| b.all_hits), [0.0, 0.0, 10.0, 0.0]);
    }
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Powerups>()
            .init_resource::<Time>()
            .init_resource::<crate::ui::PauseState>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(Round::default())
            .insert_resource(RailMap::from_seed(42));
        app.add_systems(
            Update,
            (tick_and_drop, collect)
                .chain()
                .run_if(crate::game::round_is_live)
                .run_if(crate::ui::gameplay_is_running),
        );
        app
    }
    #[test]
    fn real_drop_spawns_on_rail_and_is_collected_once_by_live_cart() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(30));
        app.update();
        let (entity, kind, position) = app
            .world_mut()
            .query::<(Entity, &Pickup, &Transform)>()
            .iter(app.world())
            .map(|(e, p, t)| (e, p.0, t.translation))
            .next()
            .expect("first rail drop");
        assert!(
            app.world()
                .resource::<RailMap>()
                .distance_to_track(position.xz())
                < 0.001
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::ZERO);
        app.world_mut().spawn((
            Player {
                id: 0,
                active: true,
                alive: true,
                score: 0,
                shot_cooldown: 0.0,
                wants_to_fire: false,
                input: None,
            },
            Transform::from_translation(position),
        ));
        app.update();
        assert!(app.world().get_entity(entity).is_err());
        let buffs = app.world().resource::<Powerups>().buffs[0];
        assert_eq!(
            if kind == Kind::Speed {
                buffs.speed
            } else {
                buffs.all_hits
            },
            10.0
        );
    }
    #[test]
    fn speed_is_non_stacking_refreshes_and_expires_while_buffs_coexist() {
        let mut buffs = Buffs::default();
        buffs.grant(Kind::Speed);
        assert_eq!(buffs.multiplier(), 1.25);
        buffs.tick(4.0);
        buffs.grant(Kind::Speed);
        buffs.grant(Kind::AllHits);
        assert_eq!(buffs.speed, 10.0);
        assert_eq!(buffs.all_hits, 10.0);
        assert_eq!(buffs.multiplier(), 1.25);
        buffs.tick(9.999);
        assert_eq!(buffs.multiplier(), 1.25);
        buffs.tick(0.0011);
        assert_eq!(buffs.multiplier(), 1.0);
        assert_eq!(buffs.all_hits, 0.0);
    }
    #[test]
    fn first_drop_at_thirty_seconds_then_random_five_to_twenty() {
        let mut drops = Powerups::default();
        assert_eq!(drops.advance(29.999), 0);
        assert_eq!(drops.advance(0.0011), 1);
        for _ in 0..100 {
            assert!((5.0..=20.0).contains(&drops.until_drop));
            let remaining = drops.until_drop;
            assert_eq!(drops.advance(remaining), 1);
        }
    }
}
