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
pub(crate) struct SeatInputs {
    pub(crate) intents: [ControlIntent; 4],
    /// Non-device controlled seats (for the headless caller). Local seats are
    /// still selected by the offline roster; an empty intent is neutral, not a bot.
    pub(crate) external: [bool; 4],
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
