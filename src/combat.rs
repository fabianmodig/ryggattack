//! Projectile firing, movement, rear-hit detection, and scoring.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::players::{PLAYER_COLORS, PLAYER_RADIUS, Player};
use crate::scene::ARENA_HALF_SIZE;
use crate::tracks::{RailFollower, RailMap, reset_cart};

const PROJECTILE_RADIUS: f32 = 0.18;

const PROJECTILE_SPEED: f32 = 13.0;

const SHOT_COOLDOWN: f32 = 0.1625;

const BACK_HIT_DOT_THRESHOLD: f32 = 0.45;

#[derive(Component)]
pub(crate) struct Projectile {
    owner: usize,
    velocity: Vec3,
    remaining_life: f32,
}

pub(crate) fn fire_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        commands.spawn((
            Projectile {
                owner: player.id,
                velocity: forward * PROJECTILE_SPEED,
                remaining_life: 2.0,
            },
            Mesh3d(meshes.add(Sphere::new(PROJECTILE_RADIUS))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: PLAYER_COLORS[player.id],
                emissive: LinearRgba::from(PLAYER_COLORS[player.id]) * 3.0,
                ..default()
            })),
            Transform::from_translation(position),
        ));
    }
}

pub(crate) fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut projectiles: Query<(Entity, &mut Projectile, &mut Transform)>,
) {
    for (entity, mut projectile, mut transform) in &mut projectiles {
        transform.translation += projectile.velocity * time.delta_secs();
        projectile.remaining_life -= time.delta_secs();
        if projectile.remaining_life <= 0.0
            || transform.translation.x.abs() > ARENA_HALF_SIZE
            || transform.translation.z.abs() > ARENA_HALF_SIZE
        {
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
            if distance > PLAYER_RADIUS + PROJECTILE_RADIUS {
                continue;
            }

            consumed.insert(projectile_entity);
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
}
