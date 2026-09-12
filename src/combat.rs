//! Missiles: firing, flight, what they strike, and the rear hits that score.

use std::collections::HashSet;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::explosions::{self, EffectAssets};
use crate::players::{PLAYER_COLORS, PLAYER_RADIUS, Player};
use crate::scene::ARENA_HALF_SIZE;
use crate::scenery::{Scenery, Transient};
use crate::tracks::{RailFollower, RailMap, reset_cart};

/// Radius a missile has to come within to strike something.
const MISSILE_RADIUS: f32 = 0.18;

const MISSILE_SPEED: f32 = 13.0;

const SHOT_COOLDOWN: f32 = 0.1625;

const BACK_HIT_DOT_THRESHOLD: f32 = 0.45;

/// Seconds between the smoke puffs a missile leaves behind.
const TRAIL_INTERVAL: f32 = 0.03;

#[derive(Component)]
pub(crate) struct Projectile {
    owner: usize,
    velocity: Vec3,
    remaining_life: f32,
    /// Time until the next puff of exhaust.
    trail: f32,
}

/// The meshes and materials every missile is built from.
#[derive(Resource)]
pub(crate) struct MissileAssets {
    body: Handle<Mesh>,
    nose: Handle<Mesh>,
    fin: Handle<Mesh>,
    exhaust: Handle<Mesh>,
    hull: Handle<StandardMaterial>,
    fins: Handle<StandardMaterial>,
    paint: [Handle<StandardMaterial>; 4],
    glow: [Handle<StandardMaterial>; 4],
}

impl MissileAssets {
    pub(crate) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            body: meshes.add(Cylinder::new(0.11, 0.46)),
            nose: meshes.add(Cone::new(0.11, 0.24)),
            fin: meshes.add(Cuboid::new(0.03, 0.36, 0.14)),
            exhaust: meshes.add(Sphere::new(0.09)),
            hull: materials.add(StandardMaterial {
                base_color: Color::srgb(0.88, 0.88, 0.86),
                metallic: 0.6,
                perceptual_roughness: 0.35,
                ..default()
            }),
            fins: materials.add(StandardMaterial {
                base_color: Color::srgb(0.18, 0.19, 0.21),
                metallic: 0.5,
                perceptual_roughness: 0.4,
                ..default()
            }),
            paint: PLAYER_COLORS.map(|color| {
                materials.add(StandardMaterial {
                    base_color: color,
                    metallic: 0.4,
                    perceptual_roughness: 0.35,
                    ..default()
                })
            }),
            glow: PLAYER_COLORS.map(|color| {
                materials.add(StandardMaterial {
                    base_color: color,
                    emissive: LinearRgba::from(color) * 8.0,
                    unlit: true,
                    ..default()
                })
            }),
        }
    }
}

pub(crate) fn fire_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<MissileAssets>,
    mut players: Query<(&mut Player, &Transform)>,
) {
    for (mut player, transform) in &mut players {
        player.shot_cooldown = (player.shot_cooldown - time.delta_secs()).max(0.0);
        if !player.wants_to_fire || player.shot_cooldown > 0.0 {
            continue;
        }

        player.shot_cooldown = SHOT_COOLDOWN;
        let forward = transform.forward().as_vec3();
        let position = transform.translation + forward * 0.9 + Vec3::Y * 0.15;
        spawn_missile(&mut commands, &assets, player.id, position, forward);
    }
}

/// A missile pointing along `forward`: the hull in the owner's colour, a pale
/// nose cone, two crossed fins, and a glowing exhaust.
fn spawn_missile(
    commands: &mut Commands,
    assets: &MissileAssets,
    owner: usize,
    position: Vec3,
    forward: Vec3,
) {
    use std::f32::consts::FRAC_PI_2;

    commands
        .spawn((
            Projectile {
                owner,
                velocity: forward * MISSILE_SPEED,
                remaining_life: 2.0,
                trail: 0.0,
            },
            Transient,
            Transform::from_translation(position).looking_to(forward, Vec3::Y),
            Visibility::default(),
        ))
        .with_children(|parent| {
            // The primitives point along +Y; the missile flies along its own -Z.
            parent.spawn((
                Mesh3d(assets.body.clone()),
                MeshMaterial3d(assets.paint[owner].clone()),
                Transform::from_rotation(Quat::from_rotation_x(FRAC_PI_2)),
            ));
            parent.spawn((
                Mesh3d(assets.nose.clone()),
                MeshMaterial3d(assets.hull.clone()),
                Transform::from_xyz(0.0, 0.0, -0.35)
                    .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
            for roll in [0.0, FRAC_PI_2] {
                parent.spawn((
                    Mesh3d(assets.fin.clone()),
                    MeshMaterial3d(assets.fins.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.17).with_rotation(Quat::from_rotation_z(roll)),
                ));
            }
            parent.spawn((
                Mesh3d(assets.exhaust.clone()),
                MeshMaterial3d(assets.glow[owner].clone()),
                Transform::from_xyz(0.0, 0.0, 0.28),
                NotShadowCaster,
            ));
        });
}

/// Fly every missile on, trailing smoke, and blow up the ones that reach a
/// wall or run out of fuel.
pub(crate) fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    effects: Res<EffectAssets>,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
) {
    for (entity, mut projectile, mut transform) in &mut projectiles {
        transform.translation += projectile.velocity * time.delta_secs();
        projectile.remaining_life -= time.delta_secs();
        projectile.trail -= time.delta_secs();
        if projectile.trail <= 0.0 {
            projectile.trail = TRAIL_INTERVAL;
            let tail = transform.translation - projectile.velocity.normalize_or_zero() * 0.32;
            explosions::puff(&mut commands, &effects, tail, 0.07, 0.28, 0.45, 0.5);
        }

        let inside = transform.translation.x.abs() <= ARENA_HALF_SIZE
            && transform.translation.z.abs() <= ARENA_HALF_SIZE;
        if projectile.remaining_life <= 0.0 || !inside {
            let impact = wall_impact(transform.translation);
            explosions::detonate(&mut commands, impact);
            commands.entity(entity).despawn();
        }
    }
}

/// Where a missile that has left the arena met the wall: its position pulled
/// back to the boundary.
fn wall_impact(position: Vec3) -> Vec3 {
    Vec3::new(
        position.x.clamp(-ARENA_HALF_SIZE, ARENA_HALF_SIZE),
        position.y,
        position.z.clamp(-ARENA_HALF_SIZE, ARENA_HALF_SIZE),
    )
}

/// Missiles fly low enough that trees and boulders are in their way.
pub(crate) fn strike_scenery(
    mut commands: Commands,
    projectiles: Query<(Entity, &Transform), With<Projectile>>,
    scenery: Query<(&Transform, &Scenery), Without<Projectile>>,
) {
    for (entity, missile) in &projectiles {
        let struck = scenery.iter().any(|(prop, prop_scenery)| {
            prop_scenery.height > missile.translation.y
                && prop.translation.xz().distance(missile.translation.xz())
                    < prop_scenery.hit_radius + MISSILE_RADIUS
        });
        if struck {
            explosions::detonate(&mut commands, missile.translation);
            commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn detect_hits(
    mut commands: Commands,
    rail_map: Res<RailMap>,
    projectiles: Query<(Entity, &Projectile, &Transform), Without<Player>>,
    mut players: Query<(&mut Player, &mut RailFollower, &mut Transform), Without<Projectile>>,
) {
    let player_snapshots: Vec<(usize, Vec3, Vec3)> = players
        .iter()
        .map(|(player, _, transform)| {
            (
                player.id,
                transform.translation,
                transform.forward().as_vec3(),
            )
        })
        .collect();
    let mut consumed = HashSet::new();
    let mut hits = Vec::new();

    for (projectile_entity, projectile, projectile_transform) in &projectiles {
        for (target_id, target_position, target_forward) in &player_snapshots {
            if projectile.owner == *target_id || consumed.contains(&projectile_entity) {
                continue;
            }
            let distance = projectile_transform.translation.distance(*target_position);
            if distance > PLAYER_RADIUS + MISSILE_RADIUS {
                continue;
            }

            consumed.insert(projectile_entity);
            explosions::detonate(&mut commands, projectile_transform.translation);
            let shot_direction = projectile.velocity.normalize_or_zero();
            if is_rear_hit(shot_direction, *target_forward) {
                hits.push((projectile.owner, *target_id));
            }
        }
    }

    for entity in consumed {
        commands.entity(entity).despawn();
    }

    for (owner_id, target_id) in hits {
        for (mut player, mut rail, mut transform) in &mut players {
            if player.id == owner_id {
                player.score += 1;
            }
            if player.id == target_id {
                reset_cart(&rail_map, target_id, &mut rail, &mut transform);
                player.shot_cooldown = 0.5;
            }
        }
    }
}

fn is_rear_hit(shot_direction: Vec3, target_forward: Vec3) -> bool {
    shot_direction
        .normalize_or_zero()
        .dot(target_forward.normalize_or_zero())
        > BACK_HIT_DOT_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shot_travelling_with_the_target_is_a_rear_hit() {
        assert!(is_rear_hit(Vec3::NEG_Z, Vec3::NEG_Z));
    }

    #[test]
    fn frontal_and_side_hits_do_not_score() {
        assert!(!is_rear_hit(Vec3::Z, Vec3::NEG_Z));
        assert!(!is_rear_hit(Vec3::X, Vec3::NEG_Z));
    }

    #[test]
    fn a_missile_past_the_wall_blows_up_on_it() {
        let impact = wall_impact(Vec3::new(ARENA_HALF_SIZE + 0.4, 0.77, 2.0));
        assert_eq!(impact, Vec3::new(ARENA_HALF_SIZE, 0.77, 2.0));
        let inside = Vec3::new(1.0, 0.77, -3.0);
        assert_eq!(wall_impact(inside), inside);
    }
}
