//! Railway generation, traversal, and rendering. Internal geometry stays in this module.

mod map;
mod path;
mod render;

pub(crate) use map::{RailMap, SimpleRng, TRACK_SPACING};
pub(crate) use path::{RailFollower, RailPath, advance_cart, reset_cart, reverse_cart, track_pose};
pub(crate) use render::spawn_tracks;
