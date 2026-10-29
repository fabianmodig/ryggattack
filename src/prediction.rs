//! Bounded, travel-only speculation. Authority owns every gameplay outcome.
//! Tick records represent actual fixed travel steps, never network message counts.
use crate::network::{RailPath, RailState};
use crate::{
    input::ControlIntent,
    network::Frame,
    powerups::Buffs,
    tracks::{RailFollower, RailMap, track_pose},
};
use bevy::prelude::*;

/// Interpolate a proven directed route (at most one seam/junction between
/// samples), using the actual path geometry, never a straight world chord.
/// Incompatible/reversed routes and large displacements explicitly snap.
pub fn interpolate_rails(
    from: &RailState,
    to: &RailState,
    alpha: f32,
) -> Option<([f32; 3], [f32; 3])> {
    let rail = interpolate_rail(from, to, alpha)?;
    let (position, forward) = track_pose(rail.path, rail.progress);
    Some((position.to_array(), forward.to_array()))
}
fn interpolate_rail(from: &RailState, to: &RailState, alpha: f32) -> Option<RailState> {
    use RailPath::*;
    let sane = |rail: &RailState| {
        rail.progress.is_finite()
            && (0.0..=1.0).contains(&rail.progress)
            && match rail.path {
                Edge { from, to } => from < 25 && to < 25 && from != to,
                Junction { from, node, to } => {
                    from < 25 && node < 25 && to < 25 && from != node && node != to && from != to
                }
            }
    };
    if !sane(from) || !sane(to) || !alpha.is_finite() {
        return None;
    }
    let paths = if from.path == to.path {
        if to.progress < from.progress {
            return None;
        }
        vec![from.path]
    } else {
        match (from.path, to.path) {
            (
                Edge { from: a, to: b },
                Junction {
                    from: c, node: d, ..
                },
            ) if a == c && b == d => vec![from.path, to.path],
            (Junction { node: b, to: c, .. }, Edge { from: d, to: e }) if b == d && c == e => {
                vec![from.path, to.path]
            }
            (Edge { from: a, to: b }, Edge { from: c, to: d }) if b == c && a != d => vec![
                from.path,
                Junction {
                    from: a,
                    node: b,
                    to: d,
                },
                to.path,
            ],
            (
                Junction { node: b, to: c, .. },
                Junction {
                    from: d, node: e, ..
                },
            ) if b == d && c == e => vec![from.path, Edge { from: b, to: c }, to.path],
            _ => return None,
        }
    };
    let mut total = 0.0;
    for (i, path) in paths.iter().enumerate() {
        let start = if i == 0 { from.progress } else { 0.0 };
        let end = if i + 1 == paths.len() {
            to.progress
        } else {
            1.0
        };
        total += (end - start) * crate::tracks::track_path_length(*path);
    }
    if total > 2.0 {
        return None;
    }
    let mut distance = total * alpha.clamp(0.0, 1.0);
    for (i, path) in paths.iter().enumerate() {
        let start = if i == 0 { from.progress } else { 0.0 };
        let end = if i + 1 == paths.len() {
            to.progress
        } else {
            1.0
        };
        let length = crate::tracks::track_path_length(*path);
        let span = (end - start) * length;
        if distance <= span || i + 1 == paths.len() {
            return Some(RailState {
                path: *path,
                progress: (start + distance / length).clamp(start, end),
                requested_turn: to.requested_turn,
            });
        }
        distance -= span;
    }
    None
}

#[derive(Clone)]
struct Travel {
    rail: RailFollower,
    buffs: Buffs,
    live: bool,
}
impl Travel {
    fn step(&mut self, map: &RailMap, intent: ControlIntent) {
        if !self.live {
            return;
        }
        let dt = crate::simulation::TICK_DURATION.as_secs_f32();
        self.rail.requested_turn =
            Vec3::new(intent.horizontal, 0.0, -intent.vertical).normalize_or_zero();
        self.buffs.tick(dt);
        crate::players::travel_step(map, &mut self.rail, self.buffs.multiplier(), dt);
    }
    fn position(&self) -> [f32; 3] {
        track_pose(self.rail.path, self.rail.progress).0.to_array()
    }
    fn rail_state(&self) -> RailState {
        RailState {
            path: self.rail.path,
            progress: self.rail.progress,
            requested_turn: self.rail.requested_turn.to_array(),
        }
    }
}

pub struct Movement {
    map: RailMap,
    local: Option<Travel>,
    remainder: f64,
    head: u64,
    history: std::collections::VecDeque<(u64, ControlIntent)>,
    last_frame: Option<u64>,
    remote: std::collections::VecDeque<(Frame, f64)>,
    remote_cursor: f64,
    jitter: f64,
    lifecycle: Option<(usize, bool, bool, [u32; 4])>,
    previous: Option<Travel>,
    correction: Option<(RailPath, f32)>,
}
impl Movement {
    /// Edges come from the adapter's validated authoritative Welcome topology.
    pub fn new(edges: Vec<(usize, usize)>) -> Self {
        Self {
            map: RailMap::from_edges(0, edges),
            local: None,
            remainder: 0.0,
            head: 0,
            history: Default::default(),
            last_frame: None,
            remote: Default::default(),
            remote_cursor: 0.0,
            jitter: 0.0,
            lifecycle: None,
            previous: None,
            correction: None,
        }
    }
    pub fn observe(&mut self, frame: &Frame, received: f64, seat: usize) {
        if self.last_frame.is_some_and(|tick| frame.tick <= tick) {
            return;
        }
        let old_visual = self.visual_rail();
        self.last_frame = Some(frame.tick);
        let live = frame
            .carts
            .iter()
            .find(|cart| cart.id == seat)
            .is_some_and(|cart| cart.alive && cart.active);
        let lifecycle = (seat, live, frame.round_finished, frame.wins);
        let boundary = self.lifecycle.is_some_and(|old| old != lifecycle);
        self.lifecycle = Some(lifecycle);
        if boundary {
            self.remote.clear();
            self.remote_cursor = 0.0;
            self.jitter = 0.0;
        }
        if boundary || self.local.is_none() || frame.simulation_tick > self.head {
            self.history.clear();
            self.head = frame.simulation_tick;
            self.remainder = 0.0;
        }
        if let Some((old, at)) = self.remote.back() {
            let expected = frame.simulation_tick.saturating_sub(old.simulation_tick) as f64 / 64.0;
            self.jitter += 0.1 * (((received - at) - expected).abs() - self.jitter);
        }
        self.remote.push_back((frame.clone(), received));
        while self.remote.len() > 16 {
            self.remote.pop_front();
        }
        self.local = frame.motion.as_ref().and_then(|motion| {
            let rail = motion.rails.get(seat)?;
            let edge = |a, b| {
                self.map
                    .edges
                    .iter()
                    .any(|&(x, y)| (x == a && y == b) || (x == b && y == a))
            };
            let valid = match rail.path {
                RailPath::Edge { from, to } => edge(from, to),
                RailPath::Junction { from, node, to } => {
                    edge(from, node) && edge(node, to) && from != to
                }
            };
            if !valid || !rail.progress.is_finite() || !(0.0..=1.0).contains(&rail.progress) {
                return None;
            }
            let cart = frame.carts.iter().find(|cart| cart.id == seat)?;
            Some(Travel {
                rail: RailFollower {
                    path: rail.path,
                    progress: rail.progress,
                    requested_turn: Vec3::from_array(rail.requested_turn),
                    collision_cooldown: 0.0,
                },
                buffs: Buffs {
                    speed: cart.speed_seconds,
                    all_hits: 0.0,
                },
                live: cart.alive && cart.active && !frame.round_finished,
            })
        });
        self.history
            .retain(|(tick, _)| *tick > frame.simulation_tick);
        if let Some(local) = &mut self.local {
            self.previous = Some(local.clone());
            for (_, intent) in &self.history {
                self.previous = Some(local.clone());
                local.step(&self.map, *intent);
            }
        }
        self.head = self.head.max(frame.simulation_tick);
        self.correction = None;
        if !boundary
            && let (Some(old), Some(new)) = (old_visual, self.visual_rail())
            && old.path == new.path
            && ((old.progress - new.progress) * crate::tracks::track_path_length(new.path)).abs()
                <= 0.5
        {
            self.correction = Some((new.path, old.progress - new.progress));
        }
    }
    pub fn advance(&mut self, delta: f64, intent: ControlIntent) {
        // At most 32 actual ticks per render / half a second of speculation.
        // Invalid clocks and background stalls never become unbounded loops.
        if !delta.is_finite()
            || !(0.0..=0.5).contains(&delta)
            || !intent.horizontal.is_finite()
            || intent.horizontal.abs() > 1.0
            || !intent.vertical.is_finite()
            || intent.vertical.abs() > 1.0
        {
            self.local = None;
            self.history.clear();
            self.remainder = 0.0;
            return;
        }
        self.remainder += delta;
        if let Some((_, residual)) = &mut self.correction {
            *residual *= (-delta as f32 / 0.1).exp();
        }
        let dt = crate::simulation::TICK_DURATION.as_secs_f64();
        while self.remainder >= dt {
            if self.history.len() >= 32 {
                self.local = None;
                self.history.clear();
                self.remainder = 0.0;
                return;
            }
            if let Some(local) = &mut self.local {
                self.previous = Some(local.clone());
                local.step(&self.map, intent);
                self.head += 1;
                self.history.push_back((self.head, intent));
            }
            self.remainder -= dt;
        }
    }
    pub fn local_position(&self) -> Option<[f32; 3]> {
        self.local.as_ref().map(Travel::position)
    }
    pub fn local_pose(&self) -> Option<([f32; 3], [f32; 3])> {
        let rail = self.visual_rail()?;
        let (position, forward) = track_pose(rail.path, rail.progress);
        Some((position.to_array(), forward.to_array()))
    }
    fn visual_rail(&self) -> Option<RailState> {
        let local = self.local.as_ref()?;
        let mut rail = local.rail_state();
        if let Some(previous) = &self.previous {
            let alpha = (self.remainder / crate::simulation::TICK_DURATION.as_secs_f64()) as f32;
            rail = interpolate_rail(&previous.rail_state(), &rail, alpha).unwrap_or(rail);
        }
        if let Some((path, residual)) = self.correction
            && rail.path == path
        {
            let corrected = rail.progress + residual;
            if (0.0..=1.0).contains(&corrected) {
                rail.progress = corrected;
            }
        }
        Some(rail)
    }
    pub fn remote_pose(&mut self, seat: usize, now: f64) -> Option<([f32; 3], [f32; 3])> {
        let (last, received) = self.remote.back()?;
        let latest = last.carts.iter().find(|cart| cart.id == seat)?;
        let authority = (latest.position, latest.forward);
        if !now.is_finite() || !latest.alive || !latest.active || last.round_finished {
            return Some(authority);
        }
        let delay = (0.125 + self.jitter * 2.0).clamp(0.125, 0.25);
        let desired = last.simulation_tick as f64 + (now - received).max(0.0) * 64.0 - delay * 64.0;
        self.remote_cursor = self
            .remote_cursor
            .max(desired)
            .min(last.simulation_tick as f64);
        for i in 1..self.remote.len() {
            let (a, _) = &self.remote[i - 1];
            let (b, _) = &self.remote[i];
            if self.remote_cursor > b.simulation_tick as f64 {
                continue;
            }
            if a.wins != b.wins
                || a.round_finished != b.round_finished
                || b.simulation_tick.saturating_sub(a.simulation_tick) > 16
            {
                return Some(authority);
            }
            let old = a.carts.iter().find(|cart| cart.id == seat)?;
            let new = b.carts.iter().find(|cart| cart.id == seat)?;
            if old.alive != new.alive || old.active != new.active {
                return Some(authority);
            }
            let from = &a.motion.as_ref()?.rails[seat];
            let to = &b.motion.as_ref()?.rails[seat];
            let span = b.simulation_tick.saturating_sub(a.simulation_tick);
            if span == 0 {
                return Some(authority);
            }
            let alpha = ((self.remote_cursor - a.simulation_tick as f64) / span as f64)
                .clamp(0.0, 1.0) as f32;
            return Some(interpolate_rails(from, to, alpha).unwrap_or(authority));
        }
        Some(authority)
    }
}
