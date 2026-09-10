//! Player state, keyboard and bot input, movement, and cart collisions.

mod visuals;

pub(crate) use visuals::spawn_players;

use bevy::prelude::*;

use crate::game::Round;
use crate::tracks::{RailFollower, RailMap, TRACK_SPACING, advance_cart, reverse_cart, track_pose};

pub(crate) const PLAYER_RADIUS: f32 = 0.55;

const PLAYER_SPEED: f32 = 4.2;

const PLAYER_COLLISION_DISTANCE: f32 = 1.05;

const PLAYER_COLLISION_COOLDOWN: f32 = 0.35;

pub(crate) const PLAYER_COLORS: [Color; 4] = [
    Color::srgb(0.20, 0.65, 1.00),
    Color::srgb(1.00, 0.30, 0.25),
    Color::srgb(0.25, 0.90, 0.40),
    Color::srgb(1.00, 0.75, 0.15),
];

#[derive(Component)]
pub(crate) struct Player {
    pub(crate) id: usize,
    pub(crate) score: u32,
    pub(crate) shot_cooldown: f32,
    pub(crate) wants_to_fire: bool,
    pub(crate) bot: bool,
}

pub(crate) fn human_input(
    keys: Res<ButtonInput<KeyCode>>,
    round: Res<Round>,
    mut players: Query<(&mut Player, &mut RailFollower)>,
) {
    for (mut player, mut rail) in &mut players {
        if player.bot {
            continue;
        }

        let x = axis(&keys, KeyCode::KeyA, KeyCode::KeyD);
        let z = axis(&keys, KeyCode::KeyW, KeyCode::KeyS);
        rail.requested_turn = if round.finished {
            Vec3::ZERO
        } else {
            Vec3::new(x, 0.0, z).normalize_or_zero()
        };
        player.wants_to_fire = !round.finished && keys.pressed(KeyCode::Space);
    }
}

pub(crate) fn bot_input(
    round: Res<Round>,
    mut players: Query<(&mut Player, &mut RailFollower, &Transform)>,
) {
    let snapshots: Vec<(usize, Vec3, Vec3)> = players
        .iter()
        .map(|(player, _, transform)| {
            (
                player.id,
                transform.translation,
                transform.forward().as_vec3(),
            )
        })
        .collect();

    for (mut bot, mut rail, transform) in &mut players {
        if !bot.bot {
            continue;
        }
        if round.finished {
            rail.requested_turn = Vec3::ZERO;
            bot.wants_to_fire = false;
            continue;
        }

        let target_id = (bot.id + 1) % snapshots.len();
        let Some((_, target_position, target_forward)) = snapshots
            .iter()
            .find(|(snapshot_id, _, _)| *snapshot_id == target_id)
            .copied()
        else {
            continue;
        };
        let ambush_point = target_position - target_forward * 3.1;
        let to_ambush = ambush_point - transform.translation;
        rail.requested_turn = to_ambush.normalize_or_zero();

        let to_target = (target_position - transform.translation).normalize_or_zero();
        let aim = transform.forward().as_vec3().dot(to_target);
        bot.wants_to_fire =
            aim > 0.985 && transform.translation.distance(target_position) < TRACK_SPACING * 2.2;
    }
}

pub(crate) fn move_players(
    time: Res<Time>,
    round: Res<Round>,
    rail_map: Res<RailMap>,
    mut players: Query<(&mut RailFollower, &mut Transform), With<Player>>,
) {
    if round.finished {
        return;
    }

    for (mut rail, mut transform) in &mut players {
        advance_cart(&rail_map, &mut rail, PLAYER_SPEED * time.delta_secs());
        let (position, direction) = track_pose(rail.path, rail.progress);
        transform.translation = position;
        transform.look_at(position + direction, Vec3::Y);
    }
}

pub(crate) fn handle_player_collisions(
    time: Res<Time>,
    round: Res<Round>,
    mut players: Query<(&mut RailFollower, &mut Transform), With<Player>>,
) {
    if round.finished {
        return;
    }

    for (mut rail, _) in &mut players {
        rail.collision_cooldown = (rail.collision_cooldown - time.delta_secs()).max(0.0);
    }

    let mut pairs = players.iter_combinations_mut();
    while let Some(
        [
            (mut left_rail, mut left_transform),
            (mut right_rail, mut right_transform),
        ],
    ) = pairs.fetch_next()
    {
        if left_rail.collision_cooldown > 0.0 || right_rail.collision_cooldown > 0.0 {
            continue;
        }

        let separation = left_transform.translation.xz() - right_transform.translation.xz();
        if separation.length_squared() > PLAYER_COLLISION_DISTANCE.powi(2) {
            continue;
        }

        reverse_cart(&mut left_rail, &mut left_transform);
        reverse_cart(&mut right_rail, &mut right_transform);
        left_rail.collision_cooldown = PLAYER_COLLISION_COOLDOWN;
        right_rail.collision_cooldown = PLAYER_COLLISION_COOLDOWN;
    }
}

fn axis(keys: &ButtonInput<KeyCode>, negative: KeyCode, positive: KeyCode) -> f32 {
    let negative = keys.pressed(negative) as i8 as f32;
    let positive = keys.pressed(positive) as i8 as f32;
    positive - negative
}
