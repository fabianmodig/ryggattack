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

// Less than half a cart length: a small bump, not a launch or a free-space teleport.
const REAR_END_PUSH_DISTANCE: f32 = 0.4;

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

/// Pure travel only: also used by speculative client steps. No contact,
/// pickup, projectile, scoring, RNG or other irreversible event is run here.
pub(crate) fn travel_step(map: &RailMap, rail: &mut RailFollower, multiplier: f32, dt: f32) {
    advance_cart(map, rail, PLAYER_SPEED * multiplier * dt);
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
        travel_step(&rail_map, &mut rail, multiplier, time.delta_secs());
        let (position, direction) = track_pose(rail.path, rail.progress);
        transform.translation = position;
        transform.look_at(position + direction, Vec3::Y);
    }
}

pub(crate) fn handle_player_collisions(
    time: Res<Time>,
    round: Res<Round>,
    rail_map: Res<RailMap>,
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

        // Directed rail continuity, not just similar headings: nearby crossing
        // or diverging junction routes are still side impacts. An overlap on
        // one directed path has no geometric front; the higher seat wins ties.
        let left_ahead = cart_is_ahead(&rail_map, &right_rail, &left_rail);
        let right_ahead = cart_is_ahead(&rail_map, &left_rail, &right_rail);
        let overlap =
            left_rail.path == right_rail.path && left_rail.progress == right_rail.progress;
        if left_ahead || right_ahead || overlap {
            let (front_rail, front_transform) = if left_ahead || (overlap && left.id > right.id) {
                (&mut left_rail, &mut left_transform)
            } else {
                (&mut right_rail, &mut right_transform)
            };
            advance_cart(&rail_map, front_rail, REAR_END_PUSH_DISTANCE);
            let (position, direction) = track_pose(front_rail.path, front_rail.progress);
            front_transform.translation = position;
            front_transform.look_at(position + direction, Vec3::Y);
        } else {
            reverse_cart(&mut left_rail, &mut left_transform);
            reverse_cart(&mut right_rail, &mut right_transform);
        }
        left_rail.collision_cooldown = PLAYER_COLLISION_COOLDOWN;
        right_rail.collision_cooldown = PLAYER_COLLISION_COOLDOWN;
    }
}

// Called only for carts already within the contact radius. Such a rear contact
// can span at most one seam: the shortest intervening path is a 1.41-unit bend,
// longer than the 1.05-unit collision diameter. Compare directed route identity
// so curves work without an arbitrary heading-angle tolerance.
fn cart_is_ahead(map: &RailMap, rear: &RailFollower, front: &RailFollower) -> bool {
    if rear.path == front.path {
        return front.progress > rear.progress;
    }
    let mut next = RailFollower {
        path: rear.path,
        progress: 1.0,
        requested_turn: rear.requested_turn,
        collision_cooldown: 0.0,
    };
    advance_cart(map, &mut next, f32::EPSILON);
    next.path == front.path
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
    fn shared_fixed_tick_rear_end_pushes_only_front_without_reversing() {
        use crate::headless::{Simulation, SimulationConfig};
        use crate::tracks::RailPath;
        for front_id in [0, 1] {
            let mut simulation = Simulation::new(SimulationConfig {
                human_seats: [true, true, false, false],
                bots: 0,
                ..default()
            })
            .unwrap();
            let path = RailPath::Edge { from: 0, to: 1 };
            let world = simulation.app.world_mut();
            for (player, mut rail, mut transform) in world
                .query::<(&Player, &mut RailFollower, &mut Transform)>()
                .iter_mut(world)
            {
                if !player.active {
                    continue;
                }
                rail.path = path;
                rail.progress = if player.id == front_id { 0.5 } else { 0.0 };
                rail.collision_cooldown = 0.0;
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
                assert_eq!(
                    rail.path, path,
                    "rear-end contact must not reverse either cart; front seat={front_id}, seat={}",
                    player.id
                );
                let ordinary_distance =
                    PLAYER_SPEED * crate::simulation::TICK_DURATION.as_secs_f32();
                let expected = (if player.id == front_id { 0.5 } else { 0.0 })
                    + (ordinary_distance + if player.id == front_id { 0.4 } else { 0.0 }) / 1.7;
                assert!(
                    (rail.progress - expected).abs() < 1e-5,
                    "only the front receives the 0.4-unit nudge"
                );
                assert_eq!(rail.collision_cooldown, PLAYER_COLLISION_COOLDOWN);
                assert!(transform.forward().dot(Vec3::X) > 0.9999);
                assert!(
                    transform
                        .translation
                        .distance(track_pose(rail.path, rail.progress).0)
                        < 1e-5
                );
            }
        }
    }

    fn collision_simulation(
        poses: [(crate::tracks::RailPath, f32); 2],
        front_id: usize,
    ) -> crate::headless::Simulation {
        use crate::headless::{Simulation, SimulationConfig};
        let mut simulation = Simulation::new(SimulationConfig {
            human_seats: [true, true, false, false],
            bots: 0,
            ..default()
        })
        .unwrap();
        let world = simulation.app.world_mut();
        for (player, mut rail, mut transform) in world
            .query::<(&Player, &mut RailFollower, &mut Transform)>()
            .iter_mut(world)
        {
            if !player.active {
                continue;
            }
            (rail.path, rail.progress) = poses[usize::from(player.id == front_id)];
            rail.collision_cooldown = 0.0;
            let (position, direction) = track_pose(rail.path, rail.progress);
            *transform = Transform::from_translation(position).looking_to(direction, Vec3::Y);
        }
        simulation
    }

    // Walk real generated rails, including edge/junction seams and bends, rather
    // than using nearby positions on disconnected routes as rear-end fixtures.
    fn collision_route_pairs() -> Vec<(crate::tracks::RailPath, crate::tracks::RailPath)> {
        let map = RailMap::from_seed(42);
        let (from, to) = map.starting_route(0);
        let mut probe = RailFollower {
            path: crate::tracks::RailPath::Edge { from, to },
            progress: 1.0,
            requested_turn: Vec3::ZERO,
            collision_cooldown: 0.0,
        };
        let mut pairs = Vec::new();
        for _ in 0..40 {
            let previous = probe.path;
            probe.progress = 1.0;
            advance_cart(&map, &mut probe, 0.0001);
            pairs.push((previous, probe.path));
        }
        assert!(
            pairs
                .iter()
                .any(|(path, _)| track_pose(*path, 0.0).1.dot(track_pose(*path, 1.0).1) < 0.1)
        );
        pairs
    }

    #[test]
    fn shared_fixed_tick_rear_end_crosses_bends_and_route_seams_without_reversal() {
        for (rear_path, front_path) in collision_route_pairs() {
            for front_id in [0, 1] {
                let mut simulation =
                    collision_simulation([(rear_path, 0.9), (front_path, 0.1)], front_id);
                let map = RailMap::from_seed(42);
                let mut expected: Vec<_> = [(rear_path, 0.9), (front_path, 0.1)]
                    .into_iter()
                    .map(|(path, progress)| {
                        let mut rail = RailFollower {
                            path,
                            progress,
                            requested_turn: Vec3::ZERO,
                            collision_cooldown: 0.0,
                        };
                        advance_cart(
                            &map,
                            &mut rail,
                            PLAYER_SPEED * crate::simulation::TICK_DURATION.as_secs_f32(),
                        );
                        rail
                    })
                    .collect();
                advance_cart(&map, &mut expected[1], 0.4);
                simulation.step();
                let world = simulation.app.world_mut();
                for (player, rail, transform) in world
                    .query::<(&Player, &RailFollower, &Transform)>()
                    .iter(world)
                {
                    if !player.active {
                        continue;
                    }
                    let wanted = &expected[usize::from(player.id == front_id)];
                    assert_eq!(
                        rail.path, wanted.path,
                        "{rear_path:?} -> {front_path:?}, front={front_id}, seat={}",
                        player.id
                    );
                    assert!((rail.progress - wanted.progress).abs() < 1e-5);
                    let (position, direction) = track_pose(wanted.path, wanted.progress);
                    assert!(transform.translation.distance(position) < 1e-5);
                    assert!(transform.forward().dot(direction) > 0.9999);
                    assert_eq!(rail.collision_cooldown, PLAYER_COLLISION_COOLDOWN);
                }
            }
        }
    }

    #[test]
    fn shared_fixed_tick_rear_end_on_bends_and_overlaps_has_one_bounded_push() {
        for (path, _) in collision_route_pairs() {
            for poses in [[(path, 0.7), (path, 0.95)], [(path, 0.5); 2]] {
                for front_id in [0, 1] {
                    // Exact overlaps use the documented higher-seat tie-break.
                    let front_id = if poses[0] == poses[1] { 1 } else { front_id };
                    let mut simulation = collision_simulation(poses, front_id);
                    let map = RailMap::from_seed(42);
                    let mut expected: Vec<_> = poses
                        .into_iter()
                        .map(|(path, progress)| RailFollower {
                            path,
                            progress,
                            requested_turn: Vec3::ZERO,
                            collision_cooldown: 0.0,
                        })
                        .collect();
                    // Stay in contact for several ticks: only the first may push.
                    for tick in 0..8 {
                        for rail in &mut expected {
                            advance_cart(
                                &map,
                                rail,
                                PLAYER_SPEED * crate::simulation::TICK_DURATION.as_secs_f32(),
                            );
                        }
                        if tick == 0 {
                            advance_cart(&map, &mut expected[1], 0.4);
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
                            let wanted = &expected[usize::from(player.id == front_id)];
                            assert_eq!(rail.path, wanted.path);
                            assert!(
                                (rail.progress - wanted.progress).abs() < 1e-5,
                                "no rapid repeat push, tick={tick}"
                            );
                            let (position, direction) = track_pose(wanted.path, wanted.progress);
                            assert!(transform.translation.distance(position) < 1e-5);
                            assert!(transform.forward().dot(direction) > 0.9999);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn shared_fixed_tick_diverging_junction_routes_remain_side_impacts() {
        let map = RailMap::from_seed(42);
        let mut tested = 0;
        for (path, next) in collision_route_pairs() {
            if !matches!(path, crate::tracks::RailPath::Edge { .. }) {
                continue;
            }
            for turn in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
                let mut other = RailFollower {
                    path,
                    progress: 1.0,
                    requested_turn: turn,
                    collision_cooldown: 0.0,
                };
                advance_cart(&map, &mut other, 0.0001);
                if other.path == next {
                    continue;
                }
                for front_id in [0, 1] {
                    let poses = [(next, 0.2), (other.path, 0.4)];
                    let mut simulation = collision_simulation(poses, front_id);
                    // These paths share an entrance and similar headings, but
                    // diverge: neither cart is ahead on the other's route.
                    simulation.step();
                    let world = simulation.app.world_mut();
                    for (player, rail, transform) in world
                        .query::<(&Player, &RailFollower, &Transform)>()
                        .iter(world)
                    {
                        if !player.active {
                            continue;
                        }
                        let (path, progress) = poses[usize::from(player.id == front_id)];
                        let mut wanted = RailFollower {
                            path,
                            progress,
                            requested_turn: Vec3::ZERO,
                            collision_cooldown: 0.0,
                        };
                        advance_cart(
                            &map,
                            &mut wanted,
                            PLAYER_SPEED * crate::simulation::TICK_DURATION.as_secs_f32(),
                        );
                        let (position, direction) = track_pose(wanted.path, wanted.progress);
                        let mut expected_transform =
                            Transform::from_translation(position).looking_to(direction, Vec3::Y);
                        reverse_cart(&mut wanted, &mut expected_transform);
                        assert_eq!(rail.path, wanted.path);
                        assert!((rail.progress - wanted.progress).abs() < 1e-5);
                        assert!(transform.translation.distance(position) < 1e-5);
                        assert!(transform.forward().dot(-direction) > 0.9999);
                        assert_eq!(rail.collision_cooldown, PLAYER_COLLISION_COOLDOWN);
                    }
                    tested += 1;
                }
            }
        }
        assert!(tested > 0);
    }

    #[test]
    fn shared_fixed_tick_rear_end_ignores_inactive_dead_and_cooling_carts() {
        let path = crate::tracks::RailPath::Edge { from: 0, to: 1 };
        for ignored_id in [0, 1] {
            for reason in 0..3 {
                let mut simulation = collision_simulation([(path, 0.0), (path, 0.5)], 1);
                let world = simulation.app.world_mut();
                for (mut player, mut rail) in world
                    .query::<(&mut Player, &mut RailFollower)>()
                    .iter_mut(world)
                {
                    if player.id != ignored_id {
                        continue;
                    }
                    match reason {
                        0 => player.active = false,
                        1 => player.alive = false,
                        _ => rail.collision_cooldown = PLAYER_COLLISION_COOLDOWN,
                    }
                }
                simulation.step();
                let world = simulation.app.world_mut();
                for (player, rail) in world.query::<(&Player, &RailFollower)>().iter(world) {
                    if player.id > 1 {
                        continue;
                    }
                    assert_eq!(rail.path, path);
                    let movement = if player.active && player.alive {
                        PLAYER_SPEED * crate::simulation::TICK_DURATION.as_secs_f32() / 1.7
                    } else {
                        0.0
                    };
                    assert!((rail.progress - (player.id as f32 * 0.5 + movement)).abs() < 1e-5);
                }
            }
        }
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
            rail.path = if player.id == 0 {
                path
            } else {
                RailPath::Edge { from: 1, to: 0 }
            };
            rail.progress = if player.id == 0 { 0.0 } else { 0.5 };
            let (position, forward) = track_pose(rail.path, rail.progress);
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
            assert_eq!(
                rail.path,
                if player.id == 0 {
                    RailPath::Edge { from: 1, to: 0 }
                } else {
                    path
                }
            );
            assert_eq!(rail.collision_cooldown, PLAYER_COLLISION_COOLDOWN);
            let expected_heading = if player.id == 0 { Vec3::NEG_X } else { Vec3::X };
            assert!(transform.forward().dot(expected_heading) > 0.99);
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
