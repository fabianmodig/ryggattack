//! Rail network generation and grid coordinates.

use std::collections::HashSet;

use bevy::prelude::*;
use web_time::{SystemTime, UNIX_EPOCH};

const TRACK_GRID_SIZE: usize = 5;

pub(crate) const TRACK_SPACING: f32 = 3.5;

const CART_HEIGHT: f32 = 0.62;

#[derive(Resource)]
pub(crate) struct RailMap {
    pub(crate) seed: u64,
    pub(super) edges: Vec<(usize, usize)>,
    pub(super) neighbors: Vec<Vec<usize>>,
}

impl RailMap {
    pub(crate) fn random() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let seed = duration.as_secs() ^ (duration.subsec_nanos() as u64).rotate_left(32);
        Self::from_seed(seed)
    }

    pub(super) fn from_seed(seed: u64) -> Self {
        let node_count = TRACK_GRID_SIZE * TRACK_GRID_SIZE;
        let mut rng = SimpleRng::new(seed);
        let mut possible_edges = grid_edges();
        rng.shuffle(&mut possible_edges);

        let mut parents: Vec<usize> = (0..node_count).collect();
        let mut selected = HashSet::new();

        // Randomized Kruskal produces a spanning tree, guaranteeing that every
        // junction belongs to one connected railway network.
        for &(left, right) in &possible_edges {
            if union(&mut parents, left, right) {
                selected.insert(normalize_edge(left, right));
            }
        }

        // A tree has dead ends. Add links until every junction has at least two
        // exits, so the no-U-turn rule never traps a cart.
        for node in 0..node_count {
            while edge_degree(&selected, node) < 2 {
                let candidates: Vec<_> = grid_neighbor_nodes(node)
                    .into_iter()
                    .flatten()
                    .filter(|neighbor| !selected.contains(&normalize_edge(node, *neighbor)))
                    .collect();
                let neighbor = candidates[rng.index(candidates.len())];
                selected.insert(normalize_edge(node, neighbor));
            }
        }

        // Extra links make each launch feel less like a pure maze and create
        // more tactical intersections and loops.
        for (left, right) in possible_edges {
            if !selected.contains(&normalize_edge(left, right)) && rng.chance(28) {
                selected.insert(normalize_edge(left, right));
            }
        }

        Self::from_edges(seed, selected.into_iter().collect())
    }

    pub(super) fn from_edges(seed: u64, mut edges: Vec<(usize, usize)>) -> Self {
        edges.sort_unstable();
        let mut neighbors = vec![Vec::new(); TRACK_GRID_SIZE * TRACK_GRID_SIZE];
        for &(left, right) in &edges {
            neighbors[left].push(right);
            neighbors[right].push(left);
        }
        for adjacent in &mut neighbors {
            adjacent.sort_unstable();
        }
        Self {
            seed,
            edges,
            neighbors,
        }
    }

    pub(crate) fn starting_route(&self, id: usize) -> (usize, usize) {
        const STARTS: [usize; 4] = [0, 24, 20, 4];
        let from = STARTS[id % STARTS.len()];
        (from, self.neighbors[from][id % self.neighbors[from].len()])
    }
}

struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0 = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        self.0
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() as usize) % length
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            values.swap(index, self.index(index + 1));
        }
    }
}

pub(super) fn rail_position(node: usize) -> Vec3 {
    let column = (node % TRACK_GRID_SIZE) as f32;
    let row = (node / TRACK_GRID_SIZE) as f32;
    let center = (TRACK_GRID_SIZE as f32 - 1.0) * 0.5;
    Vec3::new(
        (column - center) * TRACK_SPACING,
        CART_HEIGHT,
        (row - center) * TRACK_SPACING,
    )
}

fn grid_neighbor_nodes(node: usize) -> [Option<usize>; 4] {
    let row = node / TRACK_GRID_SIZE;
    let column = node % TRACK_GRID_SIZE;
    [
        (row > 0).then_some(node.saturating_sub(TRACK_GRID_SIZE)),
        (column + 1 < TRACK_GRID_SIZE).then_some(node + 1),
        (row + 1 < TRACK_GRID_SIZE).then_some(node + TRACK_GRID_SIZE),
        (column > 0).then_some(node.saturating_sub(1)),
    ]
}

pub(super) fn grid_edges() -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for row in 0..TRACK_GRID_SIZE {
        for column in 0..TRACK_GRID_SIZE {
            let node = row * TRACK_GRID_SIZE + column;
            if column + 1 < TRACK_GRID_SIZE {
                edges.push((node, node + 1));
            }
            if row + 1 < TRACK_GRID_SIZE {
                edges.push((node, node + TRACK_GRID_SIZE));
            }
        }
    }
    edges
}

fn normalize_edge(left: usize, right: usize) -> (usize, usize) {
    (left.min(right), left.max(right))
}

fn edge_degree(edges: &HashSet<(usize, usize)>, node: usize) -> usize {
    edges
        .iter()
        .filter(|(left, right)| *left == node || *right == node)
        .count()
}

fn find_root(parents: &mut [usize], node: usize) -> usize {
    if parents[node] != node {
        let parent = parents[node];
        parents[node] = find_root(parents, parent);
    }
    parents[node]
}

fn union(parents: &mut [usize], left: usize, right: usize) -> bool {
    let left_root = find_root(parents, left);
    let right_root = find_root(parents, right);
    if left_root == right_root {
        false
    } else {
        parents[right_root] = left_root;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_maps_are_connected_and_have_no_dead_ends() {
        for seed in 1..=100 {
            let map = RailMap::from_seed(seed);
            assert!(map.neighbors.iter().all(|neighbors| neighbors.len() >= 2));

            let mut visited = HashSet::from([0]);
            let mut frontier = vec![0];
            while let Some(node) = frontier.pop() {
                for &neighbor in &map.neighbors[node] {
                    if visited.insert(neighbor) {
                        frontier.push(neighbor);
                    }
                }
            }
            assert_eq!(visited.len(), TRACK_GRID_SIZE * TRACK_GRID_SIZE);
        }
    }

    #[test]
    fn different_seeds_produce_different_maps() {
        assert_ne!(RailMap::from_seed(1).edges, RailMap::from_seed(2).edges);
    }
}
