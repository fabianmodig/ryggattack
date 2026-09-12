//! Game states and round lifecycle: timer, start, and restart.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::players::{Player, Roster};
use crate::scenery::{Forest, Transient, rebuild_forest};
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

/// Everything a round reset touches, gathered so that both the start of a
/// game and a restart read the same one thing.
#[derive(SystemParam)]
pub(crate) struct RoundReset<'w, 's> {
    commands: Commands<'w, 's>,
    round: ResMut<'w, Round>,
    roster: Res<'w, Roster>,
    rail_map: Res<'w, RailMap>,
    forest: Res<'w, Forest>,
    players: Query<
        'w,
        's,
        (
            &'static mut Player,
            &'static mut RailFollower,
            &'static mut Transform,
        ),
    >,
    transient: Query<'w, 's, Entity, With<Transient>>,
}

impl RoundReset<'_, '_> {
    /// Reset the round, the scores, and every cart, and apply the current
    /// seating. This is the single point where the roster becomes gameplay
    /// state. The forest grows back too, along with clearing away missiles
    /// and wreckage.
    fn apply(&mut self) {
        *self.round = Round::default();
        for (mut player, mut rail, mut transform) in self.players.iter_mut() {
            player.score = 0;
            player.shot_cooldown = player.id as f32 * 0.12;
            player.wants_to_fire = false;
            player.input = self.roster.seat(player.id);
            reset_cart(&self.rail_map, player.id, &mut rail, &mut transform);
        }
        rebuild_forest(&mut self.commands, &self.forest, &self.transient);
    }
}

pub(crate) fn start_new_game(mut reset: RoundReset) {
    reset.apply();
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
    mut reset: RoundReset,
) {
    // Start rather than a face button, so that a pad still holding fire as the
    // clock runs out does not restart the moment it is tapped again.
    let requested = keys.just_pressed(KeyCode::KeyR)
        || pads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::Start));
    if !reset.round.finished || !requested {
        return;
    }

    reset.apply();
}
