//! Game states and round lifecycle: timer, start, and restart.

use bevy::prelude::*;

use crate::combat::Projectile;
use crate::players::{Player, Roster};
use crate::tracks::{RailFollower, RailMap, reset_cart};

pub(crate) const ROUND_SECONDS: f32 = 60.0;

#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub(crate) enum AppState {
    #[default]
    MainMenu,
    Lobby,
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

/// Reset the round, the scores, and every cart, and apply the current seating.
/// This is the single point where the roster becomes gameplay state.
fn reset_round(
    commands: &mut Commands,
    round: &mut Round,
    roster: &Roster,
    rail_map: &RailMap,
    players: &mut Query<(&mut Player, &mut RailFollower, &mut Transform)>,
    projectiles: &Query<Entity, With<Projectile>>,
) {
    *round = Round::default();
    for (mut player, mut rail, mut transform) in players.iter_mut() {
        player.score = 0;
        player.shot_cooldown = player.id as f32 * 0.12;
        player.wants_to_fire = false;
        player.input = roster.seat(player.id);
        reset_cart(rail_map, player.id, &mut rail, &mut transform);
    }
    for entity in projectiles.iter() {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn start_new_game(
    mut commands: Commands,
    mut round: ResMut<Round>,
    roster: Res<Roster>,
    rail_map: Res<RailMap>,
    mut players: Query<(&mut Player, &mut RailFollower, &mut Transform)>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    reset_round(
        &mut commands,
        &mut round,
        &roster,
        &rail_map,
        &mut players,
        &projectiles,
    );
}

pub(crate) fn tick_round(time: Res<Time>, mut round: ResMut<Round>) {
    if !round.finished {
        round.timer.tick(time.delta());
        round.finished = round.timer.is_finished();
    }
}

pub(crate) fn restart_round(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut commands: Commands,
    mut round: ResMut<Round>,
    roster: Res<Roster>,
    rail_map: Res<RailMap>,
    mut players: Query<(&mut Player, &mut RailFollower, &mut Transform)>,
    projectiles: Query<Entity, With<Projectile>>,
) {
    // Start rather than a face button, so that a pad still holding fire as the
    // clock runs out does not restart the moment it is tapped again.
    let requested = keys.just_pressed(KeyCode::KeyR)
        || pads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::Start));
    if !round.finished || !requested {
        return;
    }

    reset_round(
        &mut commands,
        &mut round,
        &roster,
        &rail_map,
        &mut players,
        &projectiles,
    );
}
