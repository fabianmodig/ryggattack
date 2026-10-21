//! CPU-side entry parity, not renderer/art parity. The offline side uses the
//! client scene Startup, Playing OnEnter, keyboard adapter and Bevy fixed loop
//! from lib::run_client; the other side starts through Room's real countdown and
//! consumes serialized Input through Room's WebSocket reader. Neither side is a
//! second Simulation::new. Menus, GPU plugins and WebSocket admission are outside
//! this fixture (admission has separate integration coverage).
use super::*;
use crate::{
    game::{self, AppState, Round},
    input::SeatInputs,
    match_settings::{Arena, MatchSettings},
    players::{self, InputSource, Player, Roster},
    powerups::{Kind, Pickup, Powerups},
    simulation::{PresentationPlugin, SimulationPlugin},
    tracks::{RailFollower, RailMap, RailPath, track_pose},
};
use bevy::{prelude::*, state::app::StatesPlugin, time::TimeUpdateStrategy};
use tungstenite::protocol::Role;

#[test]
fn offline_server_entry_tick_indexed_keyboard_intents_match_rails_and_cart_control() {
    for arena in [Arena::Forest, Arena::Volcano] {
        let mut pair = Pair::new(42, arena);
        let initial = state(pair.offline.world_mut());
        let mut fired = false;
        let mut turned = false;
        for tick in 0..192 {
            let intents = std::array::from_fn(|seat| ControlIntent {
                horizontal: if (tick / 32 + seat) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
                vertical: if tick % 64 < 32 { 1.0 } else { 0.0 },
                fire: tick % 40 == seat,
            });
            pair.step(intents);
            let now = state(pair.offline.world_mut());
            fired |= now.carts.iter().any(|c| c.shot_cooldown > 0.0);
            turned |= now.carts.iter().any(|c| c.requested_turn != Vec3::ZERO);
        }
        assert!(
            fired && turned,
            "exercise both real input adapters, not neutral coasting"
        );
        assert_ne!(
            state(pair.offline.world_mut()).carts[0].position,
            initial.carts[0].position
        );
    }
}

#[test]
fn offline_server_entry_bot_roster_and_ai_share_the_same_tick_rules() {
    let mut pair = Pair::with_bots(42, Arena::Forest, 2);
    let initial = state(pair.offline.world_mut());
    assert!(initial.carts.iter().all(|c| c.active));
    // Place one bot behind a real human on the existing straight-rail combat
    // fixture so the AI's aim/fire assertion does not rely on a chance encounter.
    pair.fixture(|world| {
        for (player, mut rail, mut transform) in world
            .query::<(&Player, &mut RailFollower, &mut Transform)>()
            .iter_mut(world)
        {
            if player.id == 0 || player.id == 2 {
                rail.path = RailPath::Edge { from: 0, to: 1 };
                rail.progress = if player.id == 0 { 0.8 } else { 0.0 };
                rail.collision_cooldown = 0.0;
                let (position, forward) = track_pose(rail.path, rail.progress);
                *transform = Transform::from_translation(position).looking_to(forward, Vec3::Y);
            }
        }
    });
    let mut bot_steered = false;
    let mut bot_fired = false;
    for _ in 0..128 {
        pair.step([ControlIntent::default(); 2]);
        let now = state(pair.offline.world_mut());
        bot_steered |= now.carts[2..]
            .iter()
            .any(|c| c.requested_turn != Vec3::ZERO);
        bot_fired |= now.carts[2..].iter().any(|c| c.shot_cooldown > 0.0);
    }
    assert!(
        bot_steered && bot_fired,
        "both actual entries must run non-vacuous bot AI"
    );
}

#[test]
fn offline_server_entry_seeded_pickups_match_positions_kinds_and_timers_past_first_drop() {
    let mut traces = Vec::new();
    for seed in [42, 123] {
        let mut pair = Pair::new(seed, Arena::Forest);
        for app in [
            &mut pair.offline,
            &mut pair.room.simulation.as_mut().unwrap().app,
        ] {
            app.init_resource::<ObservedPickups>().add_systems(
                FixedUpdate,
                observe_pickups
                    .after(crate::powerups::tick_and_drop)
                    .before(crate::powerups::collect),
            );
        }
        for tick in 0..(65 * 64) {
            pair.step([ControlIntent::default(); 2]);
            let now = state(pair.offline.world_mut());
            if tick < 30 * 64 - 2 {
                assert!(now.pickups.is_empty(), "no premature seeded drop");
            }
            // Include drops collected in their spawn tick, not just surviving
            // Snapshot counts. Observe after real spawning but before collection.
            assert_eq!(
                pair.offline.world().resource::<ObservedPickups>().0,
                pair.room
                    .simulation
                    .as_ref()
                    .unwrap()
                    .app
                    .world()
                    .resource::<ObservedPickups>()
                    .0
            );
            assert!(
                !now.finished,
                "two neutral humans must keep the random stream live"
            );
        }
        let observed = pair.offline.world().resource::<ObservedPickups>().0.clone();
        assert!(
            observed.len() >= 2,
            "must exercise random drops after the initial thirty seconds"
        );
        traces.push(observed);
    }
    assert_ne!(
        traces[0], traces[1],
        "the seed must affect actual pickup state"
    );
}

#[test]
fn offline_server_entry_physical_rear_contact_pushes_only_leader_point_four_without_reversals() {
    for front in [0, 1] {
        let mut pair = Pair::new(42, Arena::Forest);
        pair.fixture(|world| {
            place_carts(
                world,
                [
                    if front == 0 { 0.5 } else { 0.0 },
                    if front == 1 { 0.5 } else { 0.0 },
                ],
            )
        });
        assert_two_live_carts(&state(pair.offline.world_mut()));
        pair.step([ControlIntent::default(); 2]);
        let now = state(pair.offline.world_mut());
        assert_two_live_carts(&now);
        for cart in now.carts.iter().filter(|c| c.active && c.alive) {
            assert_eq!(cart.path, RailPath::Edge { from: 0, to: 1 });
            let start = if cart.id == front { 0.5 } else { 0.0 };
            let expected = start
                + (4.2 * TICK_DURATION.as_secs_f32() + if cart.id == front { 0.4 } else { 0.0 })
                    / 1.7;
            assert!(
                (cart.progress - expected).abs() < 1e-5,
                "only leading cart gets the 0.4-unit push: {cart:?}"
            );
            assert_eq!(cart.collision_cooldown, 0.35);
            assert!(cart.forward.dot(Vec3::X) > 0.9999);
            assert!(
                cart.alive,
                "physical rear contact is NOT a rear-shot elimination"
            );
        }
        assert_eq!(now.wins, [0; 4]);
    }
}

#[test]
fn offline_server_entry_keyboard_fire_creates_rear_shot_and_awards_one_round_win() {
    let mut pair = Pair::new(42, Arena::Forest);
    pair.fixture(|world| place_carts(world, [0.0, 0.8]));
    pair.step([
        ControlIntent {
            fire: true,
            ..default()
        },
        ControlIntent::default(),
    ]);
    let now = state(pair.offline.world_mut());
    assert_eq!(now.carts[0].shot_cooldown, 0.5, "real fire system ran");
    assert!(
        !now.carts[1].alive,
        "the real spawned shot hit the moving target's rear"
    );
    assert_eq!(now.wins, [1, 0, 0, 0]);
    assert!(now.finished);
    assert_eq!(now.round_winner, Some(0));
    assert_eq!(now.winner, None);
}

#[test]
fn offline_server_entry_rear_shot_eliminates_but_front_side_need_attackers_all_hits() {
    for direction in [Vec3::X, Vec3::NEG_X, Vec3::Z] {
        for buffed in [None, Some(0), Some(1)] {
            let mut pair = Pair::new(42, Arena::Forest);
            pair.fixture(|world| {
                place_carts(world, [0.0, 0.8]);
                if let Some(seat) = buffed {
                    world.resource_mut::<Powerups>().buffs[seat].grant(Kind::AllHits);
                }
                // Same real rail fixture as combat::shared_headless_tick...:
                // position a real projectile to meet the moving target this tick.
                crate::combat::parity_test_incoming_shot(world, direction);
            });
            pair.step([ControlIntent::default(); 2]);
            let now = state(pair.offline.world_mut());
            let lethal = direction == Vec3::X || buffed == Some(0);
            assert_eq!(
                !now.carts[1].alive, lethal,
                "direction={direction:?}, buff owner={buffed:?}"
            );
            assert!(now.carts[0].alive);
            assert_eq!(
                now.projectiles.len(),
                0,
                "even nonlethal hits consume the shot"
            );
            assert_eq!(now.wins, [u32::from(lethal), 0, 0, 0]);
            assert_eq!(now.finished, lethal);
            assert!(
                now.carts.iter().all(|c| c.score == 0),
                "elimination, not hit-point scoring"
            );
        }
    }
}

#[test]
fn offline_server_entry_pickup_collection_expires_and_round_reset_clears_effects() {
    let mut pair = Pair::new(42, Arena::Forest);
    pair.fixture(|world| {
        place_carts(world, [0.0, 0.8]);
        let position = track_pose(RailPath::Edge { from: 0, to: 1 }, 0.0)
            .0
            .with_y(1.15);
        for kind in [Kind::Speed, Kind::AllHits] {
            world.spawn((
                Pickup(kind),
                Transform::from_translation(position),
                crate::scenery::Transient,
            ));
        }
    });
    pair.step([ControlIntent::default(); 2]);
    let collected = state(pair.offline.world_mut());
    assert!(collected.pickups.is_empty());
    assert_eq!(collected.buffs[0], [10.0, 10.0]);
    assert_eq!(collected.buffs[1], [0.0, 0.0]);
    assert_two_live_carts(&collected);
    pair.step([ControlIntent::default(); 2]);
    let boosted = state(pair.offline.world_mut());
    assert_two_live_carts(&boosted);
    let path = RailPath::Edge { from: 0, to: 1 };
    let length = track_pose(path, 0.0).0.distance(track_pose(path, 1.0).0);
    let travelled = std::array::from_fn::<_, 2, _>(|seat| {
        assert_eq!(collected.carts[seat].path, path);
        assert_eq!(boosted.carts[seat].path, path);
        assert_eq!(boosted.carts[seat].collision_cooldown, 0.0);
        (boosted.carts[seat].progress - collected.carts[seat].progress) * length
    });
    let ordinary = 4.2 * TICK_DURATION.as_secs_f32();
    assert!((travelled[1] - ordinary).abs() < 1e-6);
    assert!((travelled[0] - ordinary * 1.25).abs() < 1e-6);
    assert!(
        (travelled[0] - travelled[1] * 1.25).abs() < 1e-6,
        "a collected Speed pickup must change actual travel by exactly 1.25x"
    );
    for _ in 0..640 {
        pair.step([ControlIntent::default(); 2]);
    }
    assert_eq!(state(pair.offline.world_mut()).buffs[0], [0.0, 0.0]);

    // Re-collect real fixtures before elimination, then exercise the automatic
    // reset (never call the reset system directly to manufacture the result).
    pair.fixture(|world| {
        place_carts(world, [0.0, 0.8]);
        let position = track_pose(RailPath::Edge { from: 0, to: 1 }, 0.0)
            .0
            .with_y(1.15);
        for kind in [Kind::Speed, Kind::AllHits] {
            world.spawn((
                Pickup(kind),
                Transform::from_translation(position),
                crate::scenery::Transient,
            ));
        }
    });
    pair.step([ControlIntent::default(); 2]);
    assert_eq!(state(pair.offline.world_mut()).buffs[0], [10.0, 10.0]);
    for win in 1..=3 {
        pair.fixture(|world| {
            place_carts(world, [0.0, 0.8]);
            crate::combat::parity_test_incoming_shot(world, Vec3::X);
        });
        pair.step([ControlIntent::default(); 2]);
        let finished = state(pair.offline.world_mut());
        assert!(finished.finished);
        assert_eq!(finished.wins, [win, 0, 0, 0]);
        assert_eq!(finished.winner, if win == 3 { Some(0) } else { None });
        if win < 3 {
            for _ in 0..95 {
                pair.step([ControlIntent::default(); 2]);
                assert!(state(pair.offline.world_mut()).finished);
                assert_eq!(
                    state(pair.offline.world_mut()).wins[0],
                    win,
                    "award once only"
                );
            }
            pair.step([ControlIntent::default(); 2]);
            let reset = state(pair.offline.world_mut());
            assert!(!reset.finished);
            assert_eq!(reset.buffs, [[0.0; 2]; 4]);
            assert_eq!(reset.until_drop, 30.0);
            assert!(reset.pickups.is_empty() && reset.projectiles.is_empty());
            assert!(reset.detonations.is_empty());
            assert_eq!(
                reset
                    .carts
                    .iter()
                    .map(|c| (c.active, c.alive))
                    .collect::<Vec<_>>(),
                vec![(true, true), (true, true), (false, false), (false, false)]
            );
            assert_eq!(reset.wins, [win, 0, 0, 0]);
        }
    }
    for _ in 0..128 {
        pair.step([ControlIntent::default(); 2]);
    }
    let won = state(pair.offline.world_mut());
    assert_eq!(won.winner, Some(0));
    assert_eq!(won.wins, [3, 0, 0, 0]);
    assert!(won.finished, "match winner stops auto-next-round");
}

#[test]
fn offline_server_entry_keyboard_selects_real_junction_outgoing_edge() {
    let mut pair = Pair::new(42, Arena::Forest);
    let edges = state(pair.offline.world_mut()).edges;
    // Find a genuine choice in the generated map, not an invented rail or a
    // requested_turn-only assertion. Select a perpendicular, non-reversing exit.
    let (from, node, to, desired) = (0..25)
        .find_map(|node| {
            let neighbors: Vec<_> = edges
                .iter()
                .filter_map(|&(a, b)| {
                    if a == node {
                        Some(b)
                    } else if b == node {
                        Some(a)
                    } else {
                        None
                    }
                })
                .collect();
            if neighbors.len() < 3 {
                return None;
            }
            neighbors.iter().find_map(|&from| {
                let incoming = track_pose(RailPath::Edge { from, to: node }, 1.0).1;
                neighbors.iter().filter(|&&to| to != from).find_map(|&to| {
                    let desired = track_pose(RailPath::Edge { from: node, to }, 0.0).1;
                    (incoming.dot(desired).abs() < 0.01).then_some((from, node, to, desired))
                })
            })
        })
        .expect("seeded map must contain a branching perpendicular turn");
    let approach = RailPath::Edge { from, to: node };
    let junction = RailPath::Junction { from, node, to };
    let exit = RailPath::Edge { from: node, to };
    let entry_position = track_pose(approach, 1.0).0;
    let &(far_from, far_to) = edges
        .iter()
        .max_by(|&&(a, b), &&(c, d)| {
            track_pose(RailPath::Edge { from: a, to: b }, 0.0)
                .0
                .distance(entry_position)
                .total_cmp(
                    &track_pose(RailPath::Edge { from: c, to: d }, 0.0)
                        .0
                        .distance(entry_position),
                )
        })
        .unwrap();
    pair.fixture(|world| {
        for (p, mut rail, mut transform) in world
            .query::<(&Player, &mut RailFollower, &mut Transform)>()
            .iter_mut(world)
        {
            if !p.active {
                continue;
            }
            rail.path = if p.id == 0 {
                approach
            } else {
                RailPath::Edge {
                    from: far_from,
                    to: far_to,
                }
            };
            rail.progress = if p.id == 0 { 0.99 } else { 0.0 };
            rail.requested_turn = Vec3::ZERO;
            rail.collision_cooldown = 0.0;
            let (position, forward) = track_pose(rail.path, rail.progress);
            *transform = Transform::from_translation(position).looking_to(forward, Vec3::Y);
        }
    });
    let intents = [
        ControlIntent {
            horizontal: desired.x,
            vertical: -desired.z,
            fire: false,
        },
        default(),
    ];
    pair.step(intents);
    let entered = state(pair.offline.world_mut());
    assert_two_live_carts(&entered);
    assert_eq!(
        entered.carts[0].path, junction,
        "keyboard intent must select the outgoing node at entry"
    );
    assert!(entered.carts[0].progress > 0.0);
    let mut exited = false;
    for _ in 0..32 {
        pair.step(intents);
        let now = state(pair.offline.world_mut());
        assert_two_live_carts(&now);
        assert_eq!(now.carts[0].collision_cooldown, 0.0);
        if now.carts[0].path == exit {
            assert!(now.carts[0].progress > 0.0);
            assert!(now.carts[0].forward.dot(desired) > 0.9999);
            exited = true;
            break;
        }
        assert_eq!(
            now.carts[0].path, junction,
            "chosen route is retained through the bend"
        );
    }
    assert!(
        exited,
        "must physically leave the junction on the requested edge"
    );
}

#[test]
fn offline_server_entry_prop_strike_consumes_missile_advances_chain_and_restores_forest() {
    let mut pair = Pair::new(42, Arena::Forest);
    let original = state(pair.offline.world_mut()).scenery;
    assert!(
        !original.is_empty(),
        "real scene Startup must create live colliders"
    );
    // Find a real struck prop, a primary prop near the missile blast's edge,
    // and a secondary outside that blast but within the primary's smaller,
    // delayed chain-one radius. No scenery is fabricated, moved or removed by
    // the fixture; only a unit combat projectile is placed.
    let (hit_prop, primary, secondary, impact) = original
        .iter()
        .find_map(|&hit_prop| {
            let impact = Vec3::from_array(hit_prop.0).with_y(0.1);
            let safe_inside = impact.x.abs() < crate::scene::ARENA_HALF_SIZE - 0.5
                && impact.z.abs() < crate::scene::ARENA_HALF_SIZE - 0.5;
            if !safe_inside {
                return None;
            }
            original.iter().find_map(|&primary| {
                let p = Vec3::from_array(primary.0);
                let distance = p.xz().distance(impact.xz());
                let radius = crate::explosions::MISSILE_BLAST_RADIUS;
                if primary == hit_prop || distance > radius + primary.1 - 0.03 {
                    return None;
                }
                original.iter().find_map(|&secondary| {
                    let q = Vec3::from_array(secondary.0);
                    (p.xz().distance(q.xz())
                        < crate::explosions::CHAIN_BLAST_RADII[1] + secondary.1 - 0.03
                        && impact.xz().distance(q.xz()) > radius + secondary.1 + 0.03)
                        .then_some((hit_prop, primary, secondary, impact))
                })
            })
        })
        .expect("seeded forest must provide a real two-stage chain fixture");
    pair.fixture(|world| crate::combat::parity_test_prop_shot(world, impact));
    let armed = state(pair.offline.world_mut());
    assert_eq!(armed.projectiles.len(), 1);
    assert!(armed.detonations.is_empty());
    pair.step([ControlIntent::default(); 2]);
    let struck = state(pair.offline.world_mut());
    assert_two_live_carts(&struck);
    assert!(
        struck.projectiles.is_empty(),
        "actual scenery contact consumes the missile"
    );
    assert!(
        !struck.scenery.contains(&hit_prop),
        "the real prop that consumed the missile must be destroyed"
    );
    assert!(
        !struck.scenery.contains(&primary),
        "the primary chain prop must be destroyed by the missile blast"
    );
    assert!(
        struck.scenery.contains(&secondary),
        "the secondary must survive the initial blast"
    );
    assert!(struck.scenery.len() < original.len());
    let primary_blast = (Vec3::from_array(primary.0) + Vec3::Y * (primary.2 * 0.35)).to_array();
    let pending = struck
        .detonations
        .iter()
        .find(|d| d.0 == primary_blast && d.1 == 1)
        .expect("destroyed primary queues a real delayed chain-one blast");
    let expected_fuse = 0.06 + Vec3::from_array(primary.0).xz().distance(impact.xz()) * 0.05;
    assert!((pending.2 - expected_fuse).abs() < 1e-6);
    assert!(pending.2 > TICK_DURATION.as_secs_f32());
    let first_fuse = pending.2;
    pair.step([ControlIntent::default(); 2]);
    let ticking = state(pair.offline.world_mut());
    let pending = ticking
        .detonations
        .iter()
        .find(|d| d.0 == primary_blast && d.1 == 1)
        .expect("positive fuse must survive another tick");
    assert_eq!(pending.2, first_fuse - TICK_DURATION.as_secs_f32());
    assert!(ticking.scenery.contains(&secondary));
    let secondary_blast =
        (Vec3::from_array(secondary.0) + Vec3::Y * (secondary.2 * 0.35)).to_array();
    let mut saw_secondary_chain = false;
    for _ in 0..64 {
        pair.step([ControlIntent::default(); 2]);
        let now = state(pair.offline.world_mut());
        assert_two_live_carts(&now);
        assert!(!now.finished);
        saw_secondary_chain |= now
            .detonations
            .iter()
            .any(|d| d.0 == secondary_blast && d.1 == 2 && d.2 > 0.0);
    }
    let cleared = state(pair.offline.world_mut());
    assert!(
        saw_secondary_chain,
        "a delayed blast must wreck the secondary and queue chain two"
    );
    assert!(!cleared.scenery.contains(&secondary));
    assert!(cleared.scenery.len() < struck.scenery.len());
    assert!(
        cleared.detonations.is_empty(),
        "the finite chain must drain"
    );
    assert_eq!(
        cleared.wins, [0; 4],
        "scenery destruction never manufactures a winner"
    );

    // End the real round with a rear hit; only the automatic transition may
    // restore the forest. Never write Round, wins, or Player::alive here.
    pair.fixture(|world| {
        place_carts(world, [0.0, 0.8]);
        crate::combat::parity_test_incoming_shot(world, Vec3::X);
    });
    pair.step([ControlIntent::default(); 2]);
    let finished = state(pair.offline.world_mut());
    assert!(finished.finished && !finished.carts[1].alive);
    assert_eq!(finished.wins, [1, 0, 0, 0]);
    for _ in 0..95 {
        pair.step([ControlIntent::default(); 2]);
        assert!(state(pair.offline.world_mut()).finished);
    }
    pair.step([ControlIntent::default(); 2]);
    let reset = state(pair.offline.world_mut());
    assert!(!reset.finished);
    assert_two_live_carts(&reset);
    assert_eq!(reset.wins, [1, 0, 0, 0]);
    assert_eq!(
        reset.scenery, original,
        "automatic reset restores every position/radius/height tuple"
    );
    assert!(reset.projectiles.is_empty() && reset.detonations.is_empty());
}

fn assert_two_live_carts(now: &GameplayState) {
    assert_eq!(
        now.carts
            .iter()
            .filter(|c| c.active)
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert_eq!(
        now.carts
            .iter()
            .filter(|c| c.alive)
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[derive(Resource, Default)]
struct ObservedPickups(Vec<(u8, [f32; 3])>, std::collections::HashSet<Entity>);

fn observe_pickups(
    pickups: Query<(Entity, &Pickup, &Transform)>,
    mut observed: ResMut<ObservedPickups>,
) {
    for (entity, pickup, transform) in &pickups {
        if observed.1.insert(entity) {
            observed.0.push((
                if pickup.0 == Kind::Speed { 0 } else { 1 },
                transform.translation.to_array(),
            ));
        }
    }
}

#[derive(Debug, PartialEq)]
struct CartState {
    id: usize,
    active: bool,
    alive: bool,
    score: u32,
    path: RailPath,
    progress: f32,
    requested_turn: Vec3,
    collision_cooldown: f32,
    position: Vec3,
    forward: Vec3,
    shot_cooldown: f32,
    wants_to_fire: bool,
}
#[derive(Debug, PartialEq)]
struct GameplayState {
    edges: Vec<(usize, usize)>,
    carts: Vec<CartState>,
    pickups: Vec<(u8, [f32; 3])>,
    // Canonical gameplay multisets; never compare entity IDs, renderer parts,
    // smoke, debris, exhaust cadence, meshes or material handles.
    scenery: Vec<([f32; 3], f32, f32)>,
    projectiles: Vec<(usize, [f32; 3], [f32; 3], f32)>,
    detonations: Vec<([f32; 3], usize, f32)>,
    buffs: [[f32; 2]; 4],
    until_drop: f32,
    wins: [u32; 4],
    finished: bool,
    winner: Option<usize>,
    round_winner: Option<usize>,
    transition: f32,
}
fn state(world: &mut World) -> GameplayState {
    let mut carts: Vec<_> = world
        .query::<(&Player, &RailFollower, &Transform)>()
        .iter(world)
        .map(|(p, r, t)| CartState {
            id: p.id,
            active: p.active,
            alive: p.alive,
            score: p.score,
            path: r.path,
            progress: r.progress,
            requested_turn: r.requested_turn,
            collision_cooldown: r.collision_cooldown,
            position: t.translation,
            forward: t.forward().as_vec3(),
            shot_cooldown: p.shot_cooldown,
            wants_to_fire: p.wants_to_fire,
        })
        .collect();
    carts.sort_by_key(|c| c.id);
    let mut pickups: Vec<_> = world
        .query::<(&Pickup, &Transform)>()
        .iter(world)
        .map(|(p, t)| {
            (
                if p.0 == Kind::Speed { 0 } else { 1 },
                t.translation.to_array(),
            )
        })
        .collect();
    pickups.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut scenery: Vec<_> = world
        .query::<(&Transform, &crate::scenery::Scenery)>()
        .iter(world)
        .map(|(t, s)| (t.translation.to_array(), s.hit_radius, s.height))
        .collect();
    scenery.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut projectiles = crate::combat::parity_test_projectiles(world);
    projectiles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut detonations = crate::explosions::parity_test_detonations(world);
    detonations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let powerups = world.resource::<Powerups>();
    let round = world.resource::<Round>();
    GameplayState {
        edges: world.resource::<RailMap>().edges.clone(),
        carts,
        pickups,
        scenery,
        projectiles,
        detonations,
        buffs: powerups.buffs.map(|b| [b.speed, b.all_hits]),
        until_drop: powerups.until_drop,
        wins: round.wins,
        finished: round.finished,
        winner: round.winner,
        round_winner: round.round_winner,
        transition: round.transition,
    }
}

fn place_carts(world: &mut World, progress: [f32; 2]) {
    for (p, mut rail, mut transform) in world
        .query::<(&Player, &mut RailFollower, &mut Transform)>()
        .iter_mut(world)
    {
        if p.active {
            rail.path = RailPath::Edge { from: 0, to: 1 };
            rail.progress = progress[p.id];
            rail.requested_turn = Vec3::ZERO;
            rail.collision_cooldown = 0.0;
            let (position, forward) = track_pose(rail.path, rail.progress);
            *transform = Transform::from_translation(position).looking_to(forward, Vec3::Y);
        }
    }
}

struct Pair {
    offline: App,
    room: Room,
    clients: [WebSocket<TcpStream>; 2],
    sequence: u64,
    tick: u64,
    now: Instant,
}
impl Pair {
    fn new(seed: u64, arena: Arena) -> Self {
        Self::with_bots(seed, arena, 0)
    }
    fn with_bots(seed: u64, arena: Arena, bots: usize) -> Self {
        let mut roster = Roster::default();
        assert_eq!(roster.join(InputSource::Wasd), Some(0));
        assert_eq!(roster.join(InputSource::Arrows), Some(1));
        let mut offline = App::new();
        // Resource/plugin/system registrations are the gameplay-relevant subset
        // of lib::run_client, NOT headless::setup or Simulation::new. Full scene
        // Startup is intentional, including its forest/collision construction.
        offline
            .add_plugins((bevy::time::TimePlugin, StatesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .init_state::<AppState>()
            .init_resource::<Round>()
            .insert_resource(Powerups::from_seed(seed.rotate_left(29)))
            .insert_resource(RailMap::from_seed(seed))
            .insert_resource(roster)
            .insert_resource(MatchSettings {
                bots,
                wins_target: 3,
                arena,
            })
            .init_resource::<crate::ui::PauseState>()
            .init_resource::<SeatInputs>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<crate::settings::VideoSettings>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Startup, crate::scene::setup)
            .add_systems(OnEnter(AppState::Playing), game::start_new_game)
            .add_plugins((SimulationPlugin, PresentationPlugin))
            .add_systems(
                PreUpdate,
                players::human_input.run_if(in_state(AppState::Playing)),
            );
        offline.update();
        offline
            .world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        offline.update();
        offline.insert_resource(TimeUpdateStrategy::ManualDuration(TICK_DURATION));

        let mut lobby = crate::rooms::Lobby::new(seed, "a".repeat(20), 10, "keyboard A".into());
        assert_eq!(lobby.join(11, "keyboard B".into()).unwrap(), 1);
        lobby
            .configure(
                10,
                crate::rooms::RoomSettings {
                    arena,
                    wins_target: 3,
                    bots,
                },
            )
            .unwrap();
        lobby.ready(10, true).unwrap();
        lobby.ready(11, true).unwrap();
        let now = Instant::now();
        let mut room = Room {
            lobby,
            peers: Default::default(),
            simulation: None,
            victory_ticks: 0,
            empty_since: None,
            reservations: [None; 4],
            credentials: Default::default(),
            last_activity: now,
        };
        for tick in 1..128 {
            room.tick_at(tick, now);
            assert!(room.simulation.is_none());
        }
        room.tick_at(128, now);
        assert_eq!(room.lobby.phase, crate::rooms::Phase::Running);
        offline.update(); // Room's countdown-start tick also takes its first step.

        let clients = std::array::from_fn(|seat| {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (server, _) = listener.accept().unwrap();
            client.set_nodelay(true).unwrap();
            client.set_nonblocking(true).unwrap();
            server.set_nodelay(true).unwrap();
            server.set_nonblocking(true).unwrap();
            // Skip admission only: Room still owns the actual framed, masked,
            // serialized Input decode/sequence/seat/submit path under test.
            room.peers[seat] = Some(Peer {
                socket: WebSocket::from_raw_socket(
                    Transport {
                        stream: server,
                        deadline: None,
                        quota: None,
                        wire: Some(WireProgress::default()),
                        http_matched: 0,
                    },
                    Role::Server,
                    None,
                ),
                participant: 10 + seat as u64,
                sequence: 0,
                last_input: now,
                budget: 0,
            });
            WebSocket::from_raw_socket(client, Role::Client, None)
        });
        let mut pair = Self {
            offline,
            room,
            clients,
            sequence: 0,
            tick: 128,
            now,
        };
        pair.assert_equal();
        pair
    }
    fn fixture(&mut self, mut setup: impl FnMut(&mut World)) {
        setup(self.offline.world_mut());
        setup(self.room.simulation.as_mut().unwrap().app.world_mut());
        self.assert_equal();
    }
    fn assert_equal(&mut self) {
        let mut local = state(self.offline.world_mut());
        let remote = state(
            self.room
                .simulation
                .as_mut()
                .expect("running room")
                .app
                .world_mut(),
        );
        for (a, b) in local.carts.iter_mut().zip(&remote.carts) {
            // Keyboard sampling normalizes once before apply_inputs normalizes
            // again; serialized axes normalize only in apply_inputs. Permit the
            // resulting last-bit difference, not a changed steering direction.
            assert!(
                a.requested_turn.distance(b.requested_turn) < 1e-6,
                "steering direction differs at tick {}: {:?} vs {:?}",
                self.tick,
                a,
                b
            );
            a.requested_turn = b.requested_turn;
        }
        assert_eq!(
            local, remote,
            "entry parity at authoritative tick {}",
            self.tick
        );
    }
    fn step(&mut self, intents: [ControlIntent; 2]) {
        self.sequence += 1;
        let schemes = [
            [
                KeyCode::KeyA,
                KeyCode::KeyD,
                KeyCode::KeyS,
                KeyCode::KeyW,
                KeyCode::Space,
            ],
            [
                KeyCode::ArrowLeft,
                KeyCode::ArrowRight,
                KeyCode::ArrowDown,
                KeyCode::ArrowUp,
                KeyCode::ControlRight,
            ],
        ];
        self.offline
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        for seat in 0..2 {
            let intent = intents[seat];
            let held = [
                intent.horizontal < 0.0,
                intent.horizontal > 0.0,
                intent.vertical < 0.0,
                intent.vertical > 0.0,
                intent.fire,
            ];
            {
                let mut keys = self
                    .offline
                    .world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>();
                for (key, pressed) in schemes[seat].into_iter().zip(held) {
                    if pressed {
                        keys.press(key);
                    }
                }
            }
            let text = serde_json::to_string(&ClientMessage::Input {
                sequence: self.sequence,
                horizontal: intent.horizontal,
                vertical: intent.vertical,
                fire: intent.fire,
            })
            .unwrap();
            let bytes = text.len() + if text.len() < 126 { 6 } else { 8 };
            self.clients[seat]
                .send(tungstenite::Message::text(text))
                .unwrap();
            // Wait for a COMPLETE local frame, without advancing simulation a
            // second time to hide transport timing. Bound failures explicitly.
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut buf = [0; MAX_INPUT_BYTES];
            loop {
                let arrived = match self.room.peers[seat]
                    .as_ref()
                    .unwrap()
                    .socket
                    .get_ref()
                    .stream
                    .peek(&mut buf)
                {
                    Ok(n) => n >= bytes,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => false,
                    Err(e) => panic!("input frame peek: {e}"),
                };
                if arrived {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "complete legitimate input frame must arrive"
                );
                std::thread::yield_now();
            }
        }
        self.tick += 1;
        self.room.tick_at(self.tick, self.now);
        self.offline.update();
        for seat in 0..2 {
            assert_eq!(
                self.room.peers[seat]
                    .as_ref()
                    .expect("input accepted")
                    .sequence,
                self.sequence
            );
            loop {
                match self.clients[seat].read() {
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(e) => panic!("draining authoritative replication: {e}"),
                }
            }
        }
        self.assert_equal();
    }
}
