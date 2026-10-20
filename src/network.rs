//! Versioned development protocol. Connection ownership, never a client seat field,
//! determines authority. This slice deliberately has no rooms or reconnect claims.
use serde::{Deserialize, Serialize};
pub const VERSION: u32 = 1;
pub const MAX_INPUT_BYTES: usize = 1024;
pub const MAX_STATE_BYTES: usize = 256 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ClientMessage {
    Hello {
        version: u32,
        name: String,
    },
    Input {
        sequence: u64,
        horizontal: f32,
        vertical: f32,
        fire: bool,
    },
}
pub fn decode_client(text: &str) -> Result<ClientMessage, &'static str> {
    if text.len() > MAX_INPUT_BYTES {
        return Err("message too large");
    }
    let msg: ClientMessage = serde_json::from_str(text).map_err(|_| "invalid protocol")?;
    match &msg {
        ClientMessage::Hello { version, name } => {
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub id: u64,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// 0 projectile, 1 speed, 2 all-hits, 3 scenery, 4 explosion.
    pub kind: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub tick: u64,
    pub ack: [u64; 4],
    pub names: [Option<String>; 4],
    pub carts: Vec<crate::headless::CartSnapshot>,
    pub wins: [u32; 4],
    pub winner: Option<usize>,
    pub round_finished: bool,
    pub transition: f32,
    pub until_drop: f32,
    pub bodies: Vec<Body>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Welcome {
        version: u32,
        seat: usize,
        edges: Vec<(usize, usize)>,
    },
    State(Frame),
    Error(String),
}
