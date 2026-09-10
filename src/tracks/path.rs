//! Straight and junction paths, route selection, and distance-based cart travel.

use bevy::prelude::*;

use super::map::{RailMap, rail_position};

pub(super) const TRACK_CORNER_RADIUS: f32 = 0.9;

#[derive(Component)]
pub(crate) struct RailFollower {
    pub(crate) path: RailPath,
    pub(crate) progress: f32,
    pub(crate) requested_turn: Vec3,
    pub(crate) collision_cooldown: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RailPath {
    Edge { from: usize, to: usize },
    Junction { from: usize, node: usize, to: usize },
}

impl RailPath {
    fn reversed(self) -> Self {
        match self {
            Self::Edge { from, to } => Self::Edge { from: to, to: from },
            Self::Junction { from, node, to } => Self::Junction {
                from: to,
                node,
                to: from,
            },
        }
    }
}

pub(super) fn junction_connections(rail_map: &RailMap, node: usize) -> Vec<(usize, usize)> {
    let neighbors = &rail_map.neighbors[node];
    let mut connections = Vec::new();
    for (index, &from) in neighbors.iter().enumerate() {
        for &to in &neighbors[index + 1..] {
            connections.push((from, to));
        }
    }
    connections
}

pub(super) fn junction_directions(from: usize, node: usize, to: usize) -> [Vec3; 2] {
    let position = rail_position(node);
    // Arc progress runs from the second direction to the first.
    [to, from].map(|neighbor| (rail_position(neighbor) - position).normalize())
}

pub(super) fn track_straight_endpoints(edge: (usize, usize)) -> [Vec3; 2] {
    let start = rail_position(edge.0);
    let end = rail_position(edge.1);
    let direction = (end - start).normalize();
    [
        start + direction * TRACK_CORNER_RADIUS,
        end - direction * TRACK_CORNER_RADIUS,
    ]
}

pub(super) fn track_corner_pose(node: usize, directions: [Vec3; 2], progress: f32) -> (Vec3, Vec3) {
    let angle = progress * std::f32::consts::FRAC_PI_2;
    let radial = -directions[0] * angle.cos() - directions[1] * angle.sin();
    let center = rail_position(node) + (directions[0] + directions[1]) * TRACK_CORNER_RADIUS;
    let tangent = directions[0] * angle.sin() - directions[1] * angle.cos();
    (center + radial * TRACK_CORNER_RADIUS, tangent)
}

fn track_path_length(path: RailPath) -> f32 {
    match path {
        RailPath::Edge { from, to } => {
            let [start, end] = track_straight_endpoints((from, to));
            start.distance(end)
        }
        RailPath::Junction { from, node, to } => {
            let directions = junction_directions(from, node, to);
            if directions[0].dot(directions[1]) < -0.99 {
                2.0 * TRACK_CORNER_RADIUS
            } else {
                std::f32::consts::FRAC_PI_2 * TRACK_CORNER_RADIUS
            }
        }
    }
}

pub(crate) fn track_pose(path: RailPath, progress: f32) -> (Vec3, Vec3) {
    let [start, end] = match path {
        RailPath::Edge { from, to } => track_straight_endpoints((from, to)),
        RailPath::Junction { from, node, to } => {
            let directions = junction_directions(from, node, to);
            if directions[0].dot(directions[1]).abs() < 0.01 {
                return track_corner_pose(node, directions, progress);
            }
            [directions[1], directions[0]]
                .map(|direction| rail_position(node) + direction * TRACK_CORNER_RADIUS)
        }
    };
    (start.lerp(end, progress), (end - start).normalize())
}

fn choose_next_rail(
    rail_map: &RailMap,
    previous: usize,
    current: usize,
    requested_turn: Vec3,
) -> usize {
    let travel_direction = (rail_position(current) - rail_position(previous)).normalize_or_zero();
    let desired_direction = if requested_turn.length_squared() > 0.01 {
        requested_turn.normalize_or_zero()
    } else {
        travel_direction
    };

    rail_map.neighbors[current]
        .iter()
        .copied()
        .filter(|candidate| *candidate != previous)
        .max_by(|left, right| {
            let left_direction =
                (rail_position(*left) - rail_position(current)).normalize_or_zero();
            let right_direction =
                (rail_position(*right) - rail_position(current)).normalize_or_zero();
            left_direction
                .dot(desired_direction)
                .total_cmp(&right_direction.dot(desired_direction))
        })
        .unwrap_or(previous)
}

pub(crate) fn advance_cart(rail_map: &RailMap, rail: &mut RailFollower, mut distance: f32) {
    while distance > 0.0 {
        let length = track_path_length(rail.path);
        let remaining = length * (1.0 - rail.progress);
        if distance < remaining {
            rail.progress += distance / length;
            break;
        }
        distance -= remaining;
        rail.progress = 0.0;
        rail.path = match rail.path {
            RailPath::Edge { from, to: node } => RailPath::Junction {
                from,
                node,
                to: choose_next_rail(rail_map, from, node, rail.requested_turn),
            },
            RailPath::Junction { node: from, to, .. } => RailPath::Edge { from, to },
        };
    }
}

pub(crate) fn reverse_cart(rail: &mut RailFollower, transform: &mut Transform) {
    rail.path = rail.path.reversed();
    rail.progress = 1.0 - rail.progress;
    rail.requested_turn = Vec3::ZERO;

    transform.rotate_y(std::f32::consts::PI);
}

pub(crate) fn reset_cart(
    rail_map: &RailMap,
    id: usize,
    rail: &mut RailFollower,
    transform: &mut Transform,
) {
    let (from, to) = rail_map.starting_route(id);
    rail.path = RailPath::Edge { from, to };
    rail.progress = 0.0;
    rail.requested_turn = Vec3::ZERO;
    rail.collision_cooldown = 0.0;
    let (position, direction) = track_pose(rail.path, 0.0);
    transform.translation = position;
    transform.look_at(position + direction, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::map::grid_edges;

    #[test]
    fn a_requested_turn_is_taken_at_a_junction() {
        let map = RailMap::from_edges(1, grid_edges());
        assert_eq!(choose_next_rail(&map, 11, 12, Vec3::NEG_Z), 7);
    }

    #[test]
    fn carts_cannot_reverse_at_a_junction() {
        let map = RailMap::from_edges(1, grid_edges());
        assert_ne!(choose_next_rail(&map, 11, 12, Vec3::NEG_X), 11);
    }

    #[test]
    fn every_junction_route_joins_its_entrance_and_exit_smoothly() {
        let adjacent = [7, 11, 13, 17];
        for mask in 0_u32..16 {
            let neighbors: Vec<_> = adjacent
                .into_iter()
                .enumerate()
                .filter_map(|(index, node)| (mask & (1 << index) != 0).then_some(node))
                .collect();
            if neighbors.len() < 2 {
                continue;
            }
            let map = RailMap::from_edges(
                1,
                neighbors.iter().map(|&neighbor| (12, neighbor)).collect(),
            );
            let connections = junction_connections(&map, 12);
            assert_eq!(
                connections.len(),
                neighbors.len() * (neighbors.len() - 1) / 2
            );
            let mut turns = 0;
            for (from, to) in connections {
                let path = RailPath::Junction { from, node: 12, to };
                let (entry, entry_heading) = track_pose(path, 0.0);
                let (exit, exit_heading) = track_pose(path, 1.0);
                let (approach, approach_heading) = track_pose(RailPath::Edge { from, to: 12 }, 1.0);
                let (departure, departure_heading) =
                    track_pose(RailPath::Edge { from: 12, to }, 0.0);
                assert!(entry.distance(approach) < 1e-5);
                assert!(exit.distance(departure) < 1e-5);
                assert!(entry_heading.dot(approach_heading) > 0.99999);
                assert!(exit_heading.dot(departure_heading) > 0.99999);
                if entry_heading.dot(exit_heading).abs() < 0.01 {
                    turns += 1;
                    assert!(
                        (track_pose(path, 0.5).1.dot(entry_heading)
                            - std::f32::consts::FRAC_1_SQRT_2)
                            .abs()
                            < 1e-5
                    );
                } else {
                    assert!(track_pose(path, 0.5).0.distance(rail_position(12)) < 1e-5);
                }
            }
            if neighbors.len() == 3 {
                assert_eq!(turns, 2);
            } else if neighbors.len() == 4 {
                assert_eq!(turns, 4);
            }
        }
    }

    #[test]
    fn cart_motion_is_continuous_and_reversible_on_every_path() {
        for seed in 1..=10 {
            let map = RailMap::from_seed(seed);
            let mut paths: Vec<_> = map
                .edges
                .iter()
                .map(|&(from, to)| RailPath::Edge { from, to })
                .collect();
            for node in 0..map.neighbors.len() {
                paths.extend(
                    junction_connections(&map, node)
                        .into_iter()
                        .map(|(from, to)| RailPath::Junction { from, node, to }),
                );
            }
            for path in paths {
                let length = track_path_length(path);
                let mut previous = track_pose(path, 0.0).0;
                for step in 0..=100 {
                    let progress = step as f32 / 100.0;
                    let (position, heading) = track_pose(path, progress);
                    let (reverse_position, reverse_heading) =
                        track_pose(path.reversed(), 1.0 - progress);
                    assert!(position.distance(reverse_position) < 1e-5);
                    assert!(heading.dot(reverse_heading) < -0.99999);
                    if step > 0 {
                        assert!((position.distance(previous) - length / 100.0).abs() < 1e-4);
                    }
                    previous = position;
                }
            }
        }
    }

    #[test]
    fn turn_is_chosen_at_junction_entry_and_kept_until_exit() {
        let map = RailMap::from_edges(1, grid_edges());
        let mut rail = RailFollower {
            path: RailPath::Edge { from: 11, to: 12 },
            progress: 0.0,
            requested_turn: Vec3::NEG_Z,
            collision_cooldown: 0.0,
        };
        let approach_length = track_path_length(rail.path);
        advance_cart(&map, &mut rail, approach_length);
        let turn = RailPath::Junction {
            from: 11,
            node: 12,
            to: 7,
        };
        assert_eq!(rail.path, turn);
        assert_eq!(rail.progress, 0.0);
        rail.requested_turn = Vec3::Z;
        advance_cart(&map, &mut rail, track_path_length(turn) * 0.5);
        assert_eq!(rail.path, turn);
        assert!((rail.progress - 0.5).abs() < 1e-5);
        advance_cart(&map, &mut rail, track_path_length(turn) * 0.5 + 0.2);
        assert_eq!(rail.path, RailPath::Edge { from: 12, to: 7 });
        assert!((rail.progress * track_path_length(rail.path) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn movement_carries_remaining_distance_across_multiple_paths() {
        let map = RailMap::from_edges(1, grid_edges());
        let follower = || RailFollower {
            path: RailPath::Edge { from: 11, to: 12 },
            progress: 0.0,
            requested_turn: Vec3::NEG_Z,
            collision_cooldown: 0.0,
        };
        let mut single_step = follower();
        let mut many_steps = follower();
        advance_cart(&map, &mut single_step, 12.0);
        for _ in 0..120 {
            advance_cart(&map, &mut many_steps, 0.1);
        }
        assert_eq!(single_step.path, many_steps.path);
        assert!((single_step.progress - many_steps.progress).abs() < 1e-4);
    }

    #[test]
    fn reversing_a_cart_preserves_position_and_flips_its_route() {
        for path in [
            RailPath::Edge { from: 11, to: 12 },
            RailPath::Junction {
                from: 11,
                node: 12,
                to: 7,
            },
            RailPath::Junction {
                from: 11,
                node: 12,
                to: 13,
            },
        ] {
            let mut rail = RailFollower {
                path,
                progress: 0.25,
                requested_turn: Vec3::NEG_Z,
                collision_cooldown: 0.0,
            };
            let (position, heading) = track_pose(path, rail.progress);
            let mut transform = Transform::from_translation(position).looking_to(heading, Vec3::Y);
            reverse_cart(&mut rail, &mut transform);
            assert_eq!(rail.path, path.reversed());
            assert!((rail.progress - 0.75).abs() < f32::EPSILON);
            assert_eq!(transform.translation, position);
            assert_eq!(rail.requested_turn, Vec3::ZERO);
            let (reversed_position, reversed_heading) = track_pose(rail.path, rail.progress);
            assert!(reversed_position.distance(position) < 1e-5);
            assert!(transform.forward().as_vec3().dot(reversed_heading) > 0.99999);
        }
    }
}
