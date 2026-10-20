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
        });
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
        self.phase = if self.humans.iter().all(|h| h.ready) {
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
            self.host = self.humans.first().map_or(0, |h| h.id);
        }
        if self.phase != Phase::Running {
            self.reset();
        }
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
