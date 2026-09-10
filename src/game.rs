//! Game states and round lifecycle: timer, start, and restart.

use bevy::prelude::*;

use crate::combat::Projectile;
use crate::players::Player;
use crate::tracks::{RailFollower, RailMap, reset_cart};

pub(crate) const ROUND_SECONDS: f32 = 60.0;

#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub(crate) enum AppState {
    #[default]
    MainMenu,
    Playing,
}

#[derive(Resource)]
pub(crate) struct Round {
    pub(crate) timer: Timer,
    pub(crate) finished: bool,
}

impl Default for Round {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ROUND_SECONDS, TimerMode::Once),
            finished: false,
        }
    }
}

pub(crate) fn start_new_game(
    mut commands: Commands,
    mut round: ResMut<Round>,
    rail_map: Res<RailMap>,
    mut players: Query<(&mut Player, &mut RailFollower, &mut Transform)>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    *round = Round::default();
    for (mut player, mut rail, mut transform) in &mut players {
        player.score = 0;
        player.shot_cooldown = player.id as f32 * 0.12;
        player.wants_to_fire = false;
        reset_cart(&rail_map, player.id, &mut rail, &mut transform);
    }
    for entity in &projectiles {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn tick_round(time: Res<Time>, mut round: ResMut<Round>) {
    if !round.finished {
        round.timer.tick(time.delta());
        round.finished = round.timer.is_finished();
    }
}

pub(crate) fn restart_round(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut round: ResMut<Round>,
    rail_map: Res<RailMap>,
    mut players: Query<(&mut Player, &mut RailFollower, &mut Transform)>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    if !round.finished || !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    *round = Round::default();
    for (mut player, mut rail, mut transform) in &mut players {
        player.score = 0;
        player.shot_cooldown = player.id as f32 * 0.12;
        player.wants_to_fire = false;
        reset_cart(&rail_map, player.id, &mut rail, &mut transform);
    }
    for entity in &projectiles {
        commands.entity(entity).despawn();
    }
}
