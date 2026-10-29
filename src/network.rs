//! Versioned development protocol. Connection ownership, never a client seat field,
//! determines authority. Reconnect uses a separate private rotating bearer credential.
use serde::{Deserialize, Serialize};
pub const VERSION: u32 = 4;
pub const MAX_INPUT_BYTES: usize = 1024;
pub fn valid_code(code: &str) -> bool {
    code.len() == 20
        && code
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn valid_credential(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub const MAX_STATE_BYTES: usize = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ClientMessage {
    Hello {
        version: u32,
        name: String,
        /// None creates a private room; a code joins an existing lobby.
        room: Option<String>,
    },
    Reconnect {
        version: u32,
        room: String,
        participant: u64,
        credential: String,
    },
    Input {
        sequence: u64,
        horizontal: f32,
        vertical: f32,
        fire: bool,
    },
    Ready(bool),
    Configure(crate::rooms::RoomSettings),
    Leave,
}
impl std::fmt::Debug for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Hello { .. } => "Hello",
            Self::Reconnect { .. } => "Reconnect { credential: [REDACTED] }",
            Self::Input { .. } => "Input",
            Self::Ready(_) => "Ready",
            Self::Configure(_) => "Configure",
            Self::Leave => "Leave",
        })
    }
}
pub fn decode_client(text: &str) -> Result<ClientMessage, &'static str> {
    if text.len() > MAX_INPUT_BYTES {
        return Err("message too large");
    }
    let msg: ClientMessage = serde_json::from_str(text).map_err(|_| "invalid protocol")?;
    match &msg {
        ClientMessage::Reconnect {
            version,
            room,
            participant,
            credential,
        } => {
            if *version != VERSION
                || !valid_code(room)
                || *participant == 0
                || !valid_credential(credential)
            {
                return Err("invalid reconnect/version");
            }
        }
        ClientMessage::Hello {
            version,
            name,
            room,
        } => {
            if room.as_ref().is_some_and(|code| !valid_code(code)) {
                return Err("invalid room code");
            }
            if *version != VERSION {
                return Err("incompatible protocol");
            }
            if name.trim().is_empty()
                || name.chars().count() > 24
                || name.chars().any(char::is_control)
            {
                return Err("invalid name (1..24 characters)");
            }
        }
        ClientMessage::Configure(settings) => {
            if !settings.valid(1) {
                return Err("invalid room settings");
            }
        }
        ClientMessage::Ready(_) | ClientMessage::Leave => {}
        ClientMessage::Input {
            horizontal,
            vertical,
            ..
        } => {
            if !horizontal.is_finite()
                || !vertical.is_finite()
                || horizontal.abs() > 1.0
                || vertical.abs() > 1.0
            {
                return Err("invalid axes");
            }
        }
    }
    Ok(msg)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PropKind {
    Pine,
    Birch,
    Bush,
    Rock,
    Stump,
    Fern,
}

/// Effective source art identity, independent of the collider envelope.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropVisual {
    pub kind: PropKind,
    pub variant: u8,
    pub scale: f32,
}

/// Presentation metadata only. Optional for compatibility with older protocol-4 servers.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplosionVisual {
    pub progress: f32,
    pub ordinal: u8,
    /// Only the central ball triggers auxiliary cosmetics; satellites never do.
    pub burst: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub id: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// 0 projectile, 1 speed, 2 all-hits, 3 scenery, 4 explosion.
    pub kind: u8,
    /// Cart seat, not a participant ID or a player name. Only projectiles.
    pub owner: Option<usize>,
    /// Only scenery; ground placement uses position.x/z and rotation.
    pub prop: Option<PropVisual>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosion: Option<ExplosionVisual>,
}
pub use crate::tracks::RailPath;

/// Additive protocol-4 travel capability. Older servers omit this field and
/// newer clients must fall back to authoritative presentation, never infer rails.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RailState {
    pub path: RailPath,
    pub progress: f32,
    pub requested_turn: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotionFrame {
    pub rails: [RailState; 4],
    pub applied: [AppliedInput; 4],
}
impl MotionFrame {
    pub fn valid(&self, tick: u64) -> bool {
        let adjacent = |a: usize, b: usize| {
            a < 25
                && b < 25
                && a != b
                && ((a / 5 == b / 5 && a.abs_diff(b) == 1) || a.abs_diff(b) == 5)
        };
        self.rails.iter().all(|rail| {
            rail.progress.is_finite()
                && (0.0..=1.0).contains(&rail.progress)
                && rail
                    .requested_turn
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1.0)
                && match rail.path {
                    RailPath::Edge { from, to } => adjacent(from, to),
                    RailPath::Junction { from, node, to } => {
                        from != to && adjacent(from, node) && adjacent(node, to)
                    }
                }
        }) && self.applied.iter().all(|input| {
            input.since_tick <= tick && input.axes.iter().all(|v| v.is_finite() && v.abs() <= 1.0)
        })
    }
}
// Optional travel capability cannot invalidate otherwise valid authority. The
// surrounding transport already bounds the complete JSON message size/depth.
fn optional_motion<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<Box<MotionFrame>>, D::Error> {
    let value = serde_json::Value::deserialize(decoder)?;
    Ok(serde_json::from_value::<MotionFrame>(value)
        .ok()
        .filter(|motion| motion.valid(u64::MAX))
        .map(Box::new))
}
/// Held control selected at the boundary of a completed simulation step.
/// Zero denotes a server/direct-simulation control, not a client sequence.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct AppliedInput {
    pub sequence: u64,
    pub since_tick: u64,
    pub axes: [f32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub tick: u64,
    pub simulation_tick: u64,
    pub ack: [u64; 4],
    pub names: [Option<String>; 4],
    pub carts: Vec<crate::headless::CartSnapshot>,
    pub wins: [u32; 4],
    pub winner: Option<usize>,
    pub round_finished: bool,
    pub transition: f32,
    pub until_drop: f32,
    pub bodies: Vec<Body>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_motion"
    )]
    pub motion: Option<Box<MotionFrame>>,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        version: u32,
        seat: usize,
        room: u64,
        participant: u64,
        code: String,
        credential: String,
        edges: Vec<(usize, usize)>,
    },
    State(Frame),
    Lobby(crate::rooms::Lobby),
    /// A recoverable lobby command rejection, not a transport failure.
    Notice(String),
    Error(String),
}
impl std::fmt::Debug for ServerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Welcome {
                room,
                participant,
                seat,
                ..
            } => f
                .debug_struct("Welcome")
                .field("room", room)
                .field("participant", participant)
                .field("seat", seat)
                .field("credential", &"[REDACTED]")
                .finish(),
            Self::State(frame) => f.debug_tuple("State").field(frame).finish(),
            Self::Lobby(lobby) => f.debug_tuple("Lobby").field(lobby).finish(),
            Self::Notice(text) => f.debug_tuple("Notice").field(text).finish(),
            Self::Error(text) => f.debug_tuple("Error").field(text).finish(),
        }
    }
}
