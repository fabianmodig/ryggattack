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
    pub(crate) active: bool,
    pub(crate) alive: bool,
    pub(crate) score: u32,
    pub(crate) shot_cooldown: f32,
    pub(crate) wants_to_fire: bool,
    /// The device driving this cart, or `None` for a bot/inactive seat. Derived from
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
    pause: Option<Res<crate::ui::PauseState>>,
    mut inputs: ResMut<crate::input::SeatInputs>,
    players: Query<&Player>,
) {
    let running = !round.finished && pause.is_none_or(crate::ui::gameplay_is_running);
    for player in &players {
        let Some(source) = player.input else {
            continue;
        };
        if !running || !player.active || !player.alive {
            inputs.intents[player.id] = default();
            inputs.local_fire_pending[player.id] = false;
            continue;
        }
        let (turn, fire) = read_source(source, &roster, &keys, &pads);
        inputs.local_fire_pending[player.id] |= fire;
        inputs.intents[player.id] = crate::input::ControlIntent {
            horizontal: turn.x,
            vertical: -turn.z,
            fire,
        };
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
    inputs: Option<Res<crate::input::SeatInputs>>,
    mut players: Query<(&mut Player, &mut RailFollower, &Transform)>,
) {
    let snapshots: Vec<(usize, Vec3, Vec3)> = players
        .iter()
        .filter(|(player, _, _)| player.active && player.alive)
        .map(|(player, _, transform)| {
            (
                player.id,
                transform.translation,
                transform.forward().as_vec3(),
            )
        })
        .collect();

    for (mut bot, mut rail, transform) in &mut players {
        if !bot.active
            || !bot.alive
            || bot.input.is_some()
            || inputs.as_ref().is_some_and(|i| i.external[bot.id])
        {
            continue;
        }
        if round.finished {
            rail.requested_turn = Vec3::ZERO;
            bot.wants_to_fire = false;
            continue;
        }

        let Some((_, target_position, target_forward)) = snapshots
            .iter()
            .filter(|(snapshot_id, _, _)| *snapshot_id != bot.id)
            .min_by(|a, b| {
                a.1.distance_squared(transform.translation)
                    .total_cmp(&b.1.distance_squared(transform.translation))
            })
            .copied()
        else {
            bot.wants_to_fire = false;
            rail.requested_turn = Vec3::ZERO;
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
    powerups: Option<Res<crate::powerups::Powerups>>,
    mut players: Query<(&Player, &mut RailFollower, &mut Transform)>,
) {
    if round.finished {
        return;
    }

    for (player, mut rail, mut transform) in &mut players {
        if !player.active || !player.alive {
            continue;
        }
        let multiplier = powerups
            .as_ref()
            .map_or(1.0, |p| p.buffs[player.id].multiplier());
        advance_cart(
            &rail_map,
            &mut rail,
            PLAYER_SPEED * multiplier * time.delta_secs(),
        );
        let (position, direction) = track_pose(rail.path, rail.progress);
        transform.translation = position;
        transform.look_at(position + direction, Vec3::Y);
    }
}

pub(crate) fn handle_player_collisions(
    time: Res<Time>,
    round: Res<Round>,
    mut players: Query<(&Player, &mut RailFollower, &mut Transform)>,
) {
    if round.finished {
        return;
    }

    for (player, mut rail, _) in &mut players {
        if !player.active || !player.alive {
            continue;
        }
        rail.collision_cooldown = (rail.collision_cooldown - time.delta_secs()).max(0.0);
    }

    let mut pairs = players.iter_combinations_mut();
    while let Some(
        [
            (left, mut left_rail, mut left_transform),
            (right, mut right_rail, mut right_transform),
        ],
    ) = pairs.fetch_next()
    {
        if !left.active || !left.alive || !right.active || !right.alive {
            continue;
        }
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
    #[test]
    fn local_input_does_not_buffer_actions_while_paused_or_round_finished() {
        use bevy::ecs::system::RunSystemOnce;
        for paused in [true, false] {
            let mut app = App::new();
            let mut keys = ButtonInput::<KeyCode>::default();
            keys.press(KeyCode::Space);
            app.insert_resource(keys)
                .insert_resource(seated(&[InputSource::Wasd]))
                .init_resource::<crate::input::SeatInputs>()
                .init_resource::<crate::ui::PauseState>()
                .insert_resource(Round {
                    finished: !paused,
                    ..default()
                });
            app.world_mut().spawn(Player {
                id: 0,
                active: true,
                alive: true,
                score: 0,
                shot_cooldown: 0.0,
                wants_to_fire: false,
                input: Some(InputSource::Wasd),
            });
            if paused {
                app.insert_resource(crate::ui::MenuInput {
                    cancel: true,
                    ..default()
                });
                app.world_mut()
                    .run_system_once(crate::ui::toggle_pause_dialog)
                    .unwrap();
            }
            app.add_systems(Update, human_input);
            app.update();
            let inputs = app.world().resource::<crate::input::SeatInputs>();
            assert!(
                !inputs.intents[0].fire && !inputs.local_fire_pending[0],
                "paused={paused}: menu or finished-round fire must not leak into gameplay"
            );
        }
    }

    #[test]
    fn a_local_fire_tap_between_ticks_is_consumed_once_not_lost() {
        let mut app = App::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Space);
        app.insert_resource(keys)
            .insert_resource(seated(&[InputSource::Wasd]))
            .init_resource::<crate::input::SeatInputs>()
            .init_resource::<Round>();
        let cart = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    active: true,
                    alive: true,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: Some(InputSource::Wasd),
                },
                RailFollower {
                    path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                    progress: 0.0,
                    requested_turn: Vec3::ZERO,
                    collision_cooldown: 0.0,
                },
            ))
            .id();
        app.add_systems(Update, human_input);
        app.update(); // A render frame with a press, but no simulation tick.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        app.update(); // Release can arrive before the next fixed tick at high refresh.
        app.add_systems(PostUpdate, crate::input::apply_inputs);
        app.update();
        assert!(
            app.world().get::<Player>(cart).unwrap().wants_to_fire,
            "the local adapter must retain a sampled tap until a tick consumes it"
        );
        app.update();
        assert!(
            !app.world().get::<Player>(cart).unwrap().wants_to_fire,
            "a released tap must not turn into held fire"
        );
    }

    #[test]
    fn shared_fixed_tick_reverses_two_colliding_live_carts() {
        use crate::headless::{Simulation, SimulationConfig};
        use crate::tracks::RailPath;
        let mut simulation = Simulation::new(SimulationConfig {
            human_seats: [true, true, false, false],
            bots: 0,
            ..default()
        })
        .unwrap();
        let world = simulation.app.world_mut();
        let path = RailPath::Edge { from: 0, to: 1 };
        for (player, mut rail, mut transform) in world
            .query::<(&Player, &mut RailFollower, &mut Transform)>()
            .iter_mut(world)
        {
            if !player.active {
                continue;
            }
            rail.path = path;
            rail.progress = player.id as f32 * 0.5;
            let (position, forward) = track_pose(path, rail.progress);
            *transform = Transform::from_translation(position).looking_to(forward, Vec3::Y);
        }
        simulation.step();
        let world = simulation.app.world_mut();
        for (player, rail, transform) in world
            .query::<(&Player, &RailFollower, &Transform)>()
            .iter(world)
        {
            if !player.active {
                continue;
            }
            assert_eq!(rail.path, RailPath::Edge { from: 1, to: 0 });
            assert_eq!(rail.collision_cooldown, PLAYER_COLLISION_COOLDOWN);
            assert!(transform.forward().dot(Vec3::NEG_X) > 0.99);
        }
    }

    #[test]
    fn device_input_does_not_mutate_authoritative_cart_before_a_tick() {
        let mut app = App::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::Space);
        app.insert_resource(keys)
            .insert_resource(seated(&[InputSource::Wasd]))
            .init_resource::<crate::input::SeatInputs>()
            .init_resource::<Round>();
        let cart = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    active: true,
                    alive: true,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: Some(InputSource::Wasd),
                },
                RailFollower {
                    path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                    progress: 0.0,
                    requested_turn: Vec3::ZERO,
                    collision_cooldown: 0.0,
                },
            ))
            .id();
        app.add_systems(Update, human_input);
        app.update();
        assert!(
            !app.world().get::<Player>(cart).unwrap().wants_to_fire,
            "device polling must publish intent, not change authoritative state"
        );
        assert_eq!(
            app.world()
                .get::<RailFollower>(cart)
                .unwrap()
                .requested_turn,
            Vec3::ZERO
        );
        assert!(app.world().resource::<crate::input::SeatInputs>().intents[0].fire);
        app.add_systems(PostUpdate, crate::input::apply_inputs);
        app.update();
        assert!(app.world().get::<Player>(cart).unwrap().wants_to_fire);
        assert_eq!(
            app.world()
                .get::<RailFollower>(cart)
                .unwrap()
                .requested_turn,
            Vec3::NEG_Z
        );
    }

    #[test]
    fn boosted_movement_is_twenty_five_percent_faster_for_humans_and_bots() {
        for input in [None, Some(InputSource::Wasd)] {
            let mut app = App::new();
            let mut powerups = crate::powerups::Powerups::default();
            powerups.buffs[0].grant(crate::powerups::Kind::Speed);
            let mut time = Time::<()>::default();
            time.advance_by(std::time::Duration::from_millis(100));
            app.insert_resource(time)
                .insert_resource(powerups)
                .insert_resource(Round::default())
                .insert_resource(RailMap::from_seed(42));
            let cart = app
                .world_mut()
                .spawn((
                    Player {
                        id: 0,
                        active: true,
                        alive: true,
                        score: 0,
                        shot_cooldown: 0.0,
                        wants_to_fire: false,
                        input,
                    },
                    RailFollower {
                        path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                        progress: 0.0,
                        requested_turn: Vec3::ZERO,
                        collision_cooldown: 0.0,
                    },
                    Transform::default(),
                ))
                .id();
            app.add_systems(Update, move_players);
            app.update();
            assert!(
                (app.world()
                    .get::<Transform>(cart)
                    .unwrap()
                    .translation
                    .distance(track_pose(crate::tracks::RailPath::Edge { from: 0, to: 1 }, 0.0).0)
                    - 0.525)
                    .abs()
                    < 0.0001
            );
        }
    }

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
    fn eliminated_carts_do_not_move() {
        let mut app = App::new();
        app.insert_resource(RailMap::random())
            .insert_resource(Round::default());
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs(1));
        app.insert_resource(time);
        let cart = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    active: true,
                    alive: false,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: None,
                },
                RailFollower {
                    path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                    progress: 0.0,
                    requested_turn: Vec3::ZERO,
                    collision_cooldown: 0.0,
                },
                Transform::default(),
            ))
            .id();
        app.add_systems(Update, move_players);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(cart).unwrap().translation,
            Vec3::ZERO
        );
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
