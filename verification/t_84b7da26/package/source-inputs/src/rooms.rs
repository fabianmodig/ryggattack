//! Private-room state machine. The service supplies identities and clock ticks.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomSettings {
    pub arena: crate::headless::Arena,
    pub wins_target: u32,
    pub bots: usize,
}
impl Default for RoomSettings {
    fn default() -> Self {
        Self {
            arena: Default::default(),
            wins_target: 3,
            bots: 3,
        }
    }
}
impl RoomSettings {
    pub fn valid(&self, humans: usize) -> bool {
        matches!(self.wins_target, 1 | 3 | 5 | 7) && self.bots <= 3 && self.bots + humans <= 4
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Lobby,
    Countdown { ticks_left: u32 },
    Running,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Human {
    pub id: u64,
    pub name: String,
    pub seat: usize,
    pub ready: bool,
    pub connected: bool,
    pub reconnect_seconds: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lobby {
    pub id: u64,
    pub code: String,
    pub host: u64,
    pub humans: Vec<Human>,
    pub settings: RoomSettings,
    pub phase: Phase,
}
impl Lobby {
    pub fn new(id: u64, code: String, participant: u64, name: String) -> Self {
        Self {
            id,
            code,
            host: participant,
            humans: vec![Human {
                id: participant,
                name,
                seat: 0,
                ready: false,
                connected: true,
                reconnect_seconds: None,
            }],
            settings: Default::default(),
            phase: Phase::Lobby,
        }
    }
    pub fn join(&mut self, id: u64, name: String) -> Result<usize, &'static str> {
        if self.phase == Phase::Running {
            return Err("match running; wait for lobby");
        }
        let seat = (0..4)
            .find(|s| self.humans.iter().all(|h| h.seat != *s))
            .ok_or("room full")?;
        self.humans.push(Human {
            id,
            name,
            seat,
            ready: false,
            connected: true,
            reconnect_seconds: None,
        });
        if !self.humans.iter().any(|h| h.id == self.host && h.connected) {
            self.host = self.humans.iter().find(|h| h.connected).map_or(0, |h| h.id);
        }
        self.settings.bots = self.settings.bots.min(4 - self.humans.len());
        self.reset();
        Ok(seat)
    }
    fn reset(&mut self) {
        self.phase = Phase::Lobby;
        for h in &mut self.humans {
            h.ready = false;
        }
    }
    pub fn ready(&mut self, id: u64, ready: bool) -> Result<(), &'static str> {
        if self.phase == Phase::Running {
            return Err("match running");
        }
        if self.humans.len() + self.settings.bots < 2 {
            return Err("solo practice is offline; add a bot or human");
        }
        self.humans
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or("unknown participant")?
            .ready = ready;
        self.phase = if self.humans.iter().all(|h| h.ready && h.connected) {
            match self.phase {
                Phase::Countdown { ticks_left } => Phase::Countdown { ticks_left },
                _ => Phase::Countdown { ticks_left: 128 },
            }
        } else {
            Phase::Lobby
        };
        Ok(())
    }
    /// One authoritative 64Hz clock tick. True only on match start.
    pub fn advance(&mut self) -> bool {
        if let Phase::Countdown { ticks_left } = &mut self.phase {
            *ticks_left -= 1;
            if *ticks_left == 0 {
                self.phase = Phase::Running;
                return true;
            }
        }
        false
    }
    pub fn finish(&mut self) {
        self.reset();
    }
    pub fn leave(&mut self, id: u64) {
        self.humans.retain(|h| h.id != id);
        if id == self.host {
            self.host = self.humans.iter().find(|h| h.connected).map_or(0, |h| h.id);
        }
        if self.phase != Phase::Running {
            self.reset();
        }
    }
    pub fn disconnect(&mut self, id: u64) {
        if let Some(h) = self.humans.iter_mut().find(|h| h.id == id) {
            h.connected = false;
            h.reconnect_seconds = Some(60);
        }
        if self.host == id {
            self.host = self.humans.iter().find(|h| h.connected).map_or(0, |h| h.id);
        }
        if self.phase != Phase::Running {
            self.reset();
        }
    }
    pub fn reconnect(&mut self, id: u64) -> Result<usize, &'static str> {
        let h = self
            .humans
            .iter_mut()
            .find(|h| h.id == id)
            .ok_or("reconnect expired; join lobby again")?;
        h.connected = true;
        h.reconnect_seconds = None;
        if self.host == 0 {
            self.host = id;
        }
        Ok(h.seat)
    }
    pub fn configure(&mut self, id: u64, settings: RoomSettings) -> Result<(), &'static str> {
        if id != self.host {
            return Err("only host can change settings");
        }
        if self.phase == Phase::Running {
            return Err("match running");
        }
        if !settings.valid(self.humans.len()) {
            return Err("invalid room settings");
        }
        self.settings = settings;
        self.reset();
        Ok(())
    }
}
#[cfg(test)]
mod resilience_tests {
    use super::*;
    #[test]
    fn fresh_join_elects_host_without_taking_reserved_identity_or_restoring_former_host() {
        let mut lobby = Lobby::new(1, "a".repeat(20), 10, "same name".into());
        lobby.disconnect(10);
        assert_eq!(lobby.host, 0);
        let seat = lobby.join(11, "same name".into()).unwrap();
        assert_eq!(lobby.host, 11);
        assert_eq!(seat, 1, "invitation admission cannot take a reserved seat");
        assert_eq!(lobby.humans[0].id, 10);
        assert!(!lobby.humans[0].connected);
        lobby.configure(11, lobby.settings.clone()).unwrap();
        assert_eq!(lobby.reconnect(10).unwrap(), 0);
        assert_eq!(
            lobby.host, 11,
            "former host cannot automatically reclaim host"
        );
    }

    #[test]
    fn fresh_join_elects_host_after_last_human_deliberately_leaves() {
        let mut lobby = Lobby::new(1, "a".repeat(20), 10, "host".into());
        lobby.leave(10);
        assert_eq!(lobby.host, 0);
        assert_eq!(lobby.join(11, "new".into()).unwrap(), 0);
        assert_eq!(lobby.host, 11);
        lobby.configure(11, lobby.settings.clone()).unwrap();
        assert!(lobby.reconnect(10).is_err());
    }

    #[test]
    fn disconnect_reserves_identity_cancels_countdown_and_elects_connected_host() {
        let mut l = Lobby::new(1, "a".repeat(20), 10, "host".into());
        l.join(11, "second".into()).unwrap();
        l.ready(10, true).unwrap();
        l.ready(11, true).unwrap();
        l.disconnect(10);
        assert_eq!(l.host, 11);
        assert_eq!(l.phase, Phase::Lobby);
        assert_eq!(l.humans.len(), 2);
        assert!(!l.humans[0].connected);
        l.ready(11, true).unwrap();
        assert_eq!(l.phase, Phase::Lobby);
        l.reconnect(10).unwrap();
        assert_eq!(l.host, 11);
        assert!(l.humans[0].connected);
        l.disconnect(11);
        assert_eq!(l.host, 10);
        l.disconnect(10);
        assert_eq!(l.host, 0);
    }
}
