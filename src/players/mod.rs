//! Player state, device and bot input, movement, and cart collisions.

mod roster;
mod visuals;

pub(crate) use roster::{InputSource, Roster};
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
    /// The device driving this cart, or `None` for a bot. Derived from
    /// [`Roster`] once per round in `game::reset_round`.
    pub(crate) input: Option<InputSource>,
}

/// Steering and fire keys for one keyboard scheme.
#[derive(Clone, Copy)]
struct KeyBindings {
    left: KeyCode,
    right: KeyCode,
    up: KeyCode,
    down: KeyCode,
    fire: KeyCode,
}

const WASD: KeyBindings = KeyBindings {
    left: KeyCode::KeyA,
    right: KeyCode::KeyD,
    up: KeyCode::KeyW,
    down: KeyCode::KeyS,
    fire: KeyCode::Space,
};

const ARROWS: KeyBindings = KeyBindings {
    left: KeyCode::ArrowLeft,
    right: KeyCode::ArrowRight,
    up: KeyCode::ArrowUp,
    down: KeyCode::ArrowDown,
    fire: KeyCode::ControlRight,
};

/// Bevy's own gamepad deadzone defaults to 0.05, which is small enough that a
/// worn stick would keep picking junctions on its own.
const PAD_DEADZONE: f32 = 0.5;

pub(crate) fn human_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    roster: Res<Roster>,
    round: Res<Round>,
    mut players: Query<(&mut Player, &mut RailFollower)>,
) {
    for (mut player, mut rail) in &mut players {
        let Some(source) = player.input else {
            continue;
        };

        let (turn, fire) = read_source(source, &roster, &keys, &pads);
        rail.requested_turn = if round.finished { Vec3::ZERO } else { turn };
        player.wants_to_fire = !round.finished && fire;
    }
}

fn read_source(
    source: InputSource,
    roster: &Roster,
    keys: &ButtonInput<KeyCode>,
    pads: &Query<&Gamepad>,
) -> (Vec3, bool) {
    match source {
        InputSource::Wasd | InputSource::Arrows => {
            read_keys(keys, keyboard_schemes(source, roster))
        }
        // A disconnected pad keeps its entity but loses its `Gamepad`, so the
        // cart coasts through junctions instead of the lookup panicking.
        InputSource::Pad(entity) => match pads.get(entity) {
            Ok(pad) => {
                let axes = pad_axes(pad);
                let fire =
                    pad.pressed(GamepadButton::South) || pad.pressed(GamepadButton::RightTrigger2);
                (turn_from_axes(axes.x, axes.y), fire)
            }
            Err(_) => (Vec3::ZERO, false),
        },
    }
}

/// The keyboard schemes one seat listens to. A lone keyboard player drives with
/// WASD and the arrow keys interchangeably; once both seats are claimed each
/// scheme controls only its own cart.
fn keyboard_schemes(source: InputSource, roster: &Roster) -> &'static [KeyBindings] {
    const BOTH: [KeyBindings; 2] = [WASD, ARROWS];
    const WASD_ONLY: [KeyBindings; 1] = [WASD];
    const ARROWS_ONLY: [KeyBindings; 1] = [ARROWS];

    let other = match source {
        InputSource::Wasd => InputSource::Arrows,
        _ => InputSource::Wasd,
    };
    if roster.seat_of(other).is_none() {
        return &BOTH;
    }
    match source {
        InputSource::Wasd => &WASD_ONLY,
        _ => &ARROWS_ONLY,
    }
}

fn read_keys(keys: &ButtonInput<KeyCode>, schemes: &[KeyBindings]) -> (Vec3, bool) {
    let mut axes = Vec2::ZERO;
    let mut fire = false;
    for scheme in schemes {
        axes.x += axis(keys, scheme.left, scheme.right);
        axes.y += axis(keys, scheme.down, scheme.up);
        fire |= keys.pressed(scheme.fire);
    }
    (turn_from_axes(axes.x, axes.y), fire)
}

pub(crate) fn pad_axes(pad: &Gamepad) -> Vec2 {
    let stick = pad.left_stick();
    let stick = if stick.length_squared() > PAD_DEADZONE * PAD_DEADZONE {
        stick
    } else {
        Vec2::ZERO
    };
    pad.dpad() + stick
}

/// The camera looks down `-Z`, so "up" on any device is `-Z` in the world. Every
/// scheme funnels through here, which is what keeps them equivalent.
fn turn_from_axes(x: f32, up: f32) -> Vec3 {
    Vec3::new(x, 0.0, -up).normalize_or_zero()
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
        if bot.input.is_some() {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn seated(sources: &[InputSource]) -> Roster {
        let mut roster = Roster::default();
        for source in sources {
            roster.join(*source);
        }
        roster
    }

    fn read(keys: &ButtonInput<KeyCode>, source: InputSource, roster: &Roster) -> (Vec3, bool) {
        read_keys(keys, keyboard_schemes(source, roster))
    }

    #[test]
    fn up_steers_towards_negative_z_on_every_scheme() {
        assert_eq!(turn_from_axes(0.0, 1.0), Vec3::NEG_Z);
        assert_eq!(turn_from_axes(1.0, 0.0), Vec3::X);
        assert_eq!(turn_from_axes(0.0, 0.0), Vec3::ZERO);
    }

    #[test]
    fn wasd_and_arrows_produce_the_same_direction() {
        let roster = seated(&[InputSource::Wasd, InputSource::Arrows]);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        let wasd = read(&keys, InputSource::Wasd, &roster).0;

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        let arrows = read(&keys, InputSource::Arrows, &roster).0;

        assert_eq!(wasd, Vec3::NEG_Z);
        assert_eq!(wasd, arrows);
    }

    #[test]
    fn a_lone_keyboard_player_can_use_either_scheme() {
        let roster = seated(&[InputSource::Wasd]);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        assert_eq!(read(&keys, InputSource::Wasd, &roster).0, Vec3::NEG_Z);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ControlRight);
        assert!(read(&keys, InputSource::Wasd, &roster).1);
    }

    #[test]
    fn a_claimed_arrows_seat_stops_the_fallback() {
        let roster = seated(&[InputSource::Wasd, InputSource::Arrows]);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        assert_eq!(read(&keys, InputSource::Wasd, &roster).0, Vec3::ZERO);
        assert_eq!(read(&keys, InputSource::Arrows, &roster).0, Vec3::NEG_Z);
    }

    #[test]
    fn each_keyboard_seat_has_its_own_fire_key() {
        let roster = seated(&[InputSource::Wasd, InputSource::Arrows]);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Space);
        assert!(read(&keys, InputSource::Wasd, &roster).1);
        assert!(!read(&keys, InputSource::Arrows, &roster).1);

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ControlRight);
        assert!(read(&keys, InputSource::Arrows, &roster).1);
        assert!(!read(&keys, InputSource::Wasd, &roster).1);
    }

    #[test]
    fn diagonals_are_normalized() {
        let roster = seated(&[InputSource::Wasd, InputSource::Arrows]);
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
        assert!((read(&keys, InputSource::Wasd, &roster).0.length() - 1.0).abs() < 1e-5);
    }
}
