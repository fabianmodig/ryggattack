//! Railway generation, traversal, and rendering. Internal geometry stays in this module.

mod map;
mod path;
mod render;

pub(crate) use map::{RailMap, SimpleRng, TRACK_SPACING};
pub use path::RailPath;
pub(crate) use path::track_path_length;
pub(crate) use path::{RailFollower, advance_cart, reset_cart, reverse_cart, track_pose};
pub(crate) use render::spawn_tracks;
