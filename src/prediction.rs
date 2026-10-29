//! Bounded, travel-only speculation. Authority owns every gameplay outcome.
//! Tick records represent actual fixed travel steps, never network message counts.
use crate::{input::ControlIntent, network::Frame, powerups::Buffs, tracks::{RailFollower, RailMap, track_pose}};
use bevy::prelude::*;

#[derive(Clone)]
struct Travel {
    rail: RailFollower,
    buffs: Buffs,
    live: bool,
}
impl Travel {
    fn step(&mut self, map: &RailMap, intent: ControlIntent) {
        if !self.live { return; }
        let dt = crate::simulation::TICK_DURATION.as_secs_f32();
        self.rail.requested_turn = Vec3::new(intent.horizontal, 0.0, -intent.vertical).normalize_or_zero();
        self.buffs.tick(dt);
        crate::players::travel_step(map, &mut self.rail, self.buffs.multiplier(), dt);
    }
    fn position(&self) -> [f32; 3] { track_pose(self.rail.path, self.rail.progress).0.to_array() }
}

pub struct Movement {
    map: RailMap,
    local: Option<Travel>,
    remainder: f64,
    head: u64,
    history: std::collections::VecDeque<(u64, ControlIntent)>,
    last_frame: Option<u64>,
}
impl Movement {
    /// Edges come from the adapter's validated authoritative Welcome topology.
    pub fn new(edges: Vec<(usize, usize)>) -> Self {
        Self { map: RailMap::from_edges(0, edges), local: None, remainder: 0.0,
            head: 0, history: Default::default(), last_frame: None }
    }
    pub fn observe(&mut self, frame: &Frame, _received: f64, seat: usize) {
        if self.last_frame.is_some_and(|tick| frame.tick <= tick) { return; }
        self.last_frame = Some(frame.tick);
        self.local = frame.motion.as_ref().and_then(|motion| {
            let rail = motion.rails.get(seat)?;
            let cart = frame.carts.iter().find(|cart| cart.id == seat)?;
            Some(Travel {
                rail: RailFollower { path: rail.path, progress: rail.progress,
                    requested_turn: Vec3::from_array(rail.requested_turn), collision_cooldown: 0.0 },
                buffs: Buffs { speed: cart.speed_seconds, all_hits: 0.0 },
                live: cart.alive && cart.active && !frame.round_finished,
            })
        });
        self.history.retain(|(tick, _)| *tick > frame.simulation_tick);
        if let Some(local) = &mut self.local {
            for (_, intent) in &self.history { local.step(&self.map, *intent); }
        }
        self.head = self.head.max(frame.simulation_tick);
    }
    pub fn advance(&mut self, delta: f64, intent: ControlIntent) {
        // At most 32 actual ticks per render / half a second of speculation.
        // Invalid clocks and background stalls never become unbounded loops.
        if !delta.is_finite() || !(0.0..=0.5).contains(&delta)
            || !intent.horizontal.is_finite() || intent.horizontal.abs() > 1.0
            || !intent.vertical.is_finite() || intent.vertical.abs() > 1.0
        {
            self.local = None;
            self.history.clear();
            self.remainder = 0.0;
            return;
        }
        self.remainder += delta;
        let dt = crate::simulation::TICK_DURATION.as_secs_f64();
        while self.remainder >= dt {
            if self.history.len() >= 32 {
                self.local = None;
                self.history.clear();
                self.remainder = 0.0;
                return;
            }
            if let Some(local) = &mut self.local {
                local.step(&self.map, intent);
                self.head += 1;
                self.history.push_back((self.head, intent));
            }
            self.remainder -= dt;
        }
    }
    pub fn local_position(&self) -> Option<[f32; 3]> { self.local.as_ref().map(Travel::position) }
}
