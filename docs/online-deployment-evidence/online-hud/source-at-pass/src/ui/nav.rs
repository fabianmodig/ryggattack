//! One device-agnostic reading of what the menus were asked to do this frame.
//!
//! Menus answer to the arrow keys, WASD, Enter, and Escape as well as to a
//! gamepad's d-pad, left stick, and face buttons. Collecting that here lets
//! every screen read a single resource instead of repeating the device list.

use bevy::prelude::*;

/// How far a stick must travel before it counts as a direction.
const STICK_THRESHOLD: f32 = 0.5;

/// Which way a screen lays its buttons out, and so which pair of directions
/// steps through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavAxis {
    Vertical,
    Horizontal,
}

/// One frame of menu intent, merged across the keyboard and every pad. Every
/// field is an edge, so holding a direction or a button acts only once.
#[derive(Resource, Default, Debug)]
pub(crate) struct MenuInput {
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    /// Enter, Space, or the pad's south face button.
    pub(crate) accept: bool,
    /// Escape or the pad's east face button.
    pub(crate) cancel: bool,
    /// The pad's Start button, which has no keyboard counterpart.
    pub(crate) start: bool,
}

impl MenuInput {
    /// `-1` to step towards the first button, `1` towards the last, `0` to stay.
    pub(crate) fn step(&self, axis: NavAxis) -> i32 {
        let (backwards, forwards) = match axis {
            NavAxis::Vertical => (self.up, self.down),
            NavAxis::Horizontal => (self.left, self.right),
        };
        forwards as i32 - backwards as i32
    }

    /// Start confirms the highlighted button like the south face button does.
    /// Only the pause dialog has to tell the two apart, so the rest of the menu
    /// code asks this instead of reading the fields.
    pub(crate) fn accepted(&self) -> bool {
        self.accept || self.start
    }
}

/// Last frame's direction per pad. Sticks are analog and produce no press edge
/// of their own, so the edge is found here rather than read from the device.
#[derive(Resource, Default)]
pub(crate) struct StickLatch {
    previous: Vec<(Entity, IVec2)>,
}

impl StickLatch {
    /// Record `intent` for `pad` and report the axes that just crossed into it.
    fn edge(&mut self, pad: Entity, intent: IVec2) -> IVec2 {
        match self.previous.iter_mut().find(|(known, _)| *known == pad) {
            Some((_, previous)) => {
                let crossed = IVec2::new(
                    axis_edge(previous.x, intent.x),
                    axis_edge(previous.y, intent.y),
                );
                *previous = intent;
                crossed
            }
            None => {
                // First sighting: adopt the current direction, so a stick that
                // was already held does not read as a fresh press.
                self.previous.push((pad, intent));
                IVec2::ZERO
            }
        }
    }

    fn forget_absent(&mut self, present: &[Entity]) {
        self.previous.retain(|(pad, _)| present.contains(pad));
    }
}

pub(crate) fn gather_menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<(Entity, &Gamepad)>,
    mut latch: ResMut<StickLatch>,
    mut input: ResMut<MenuInput>,
) {
    *input = MenuInput {
        up: keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]),
        down: keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]),
        left: keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]),
        right: keys.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD]),
        accept: keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]),
        cancel: keys.just_pressed(KeyCode::Escape),
        start: false,
    };

    let connected: Vec<Entity> = pads.iter().map(|(entity, _)| entity).collect();
    latch.forget_absent(&connected);

    for (entity, pad) in &pads {
        let stepped = latch.edge(entity, direction(pad));
        input.up |= stepped.y > 0;
        input.down |= stepped.y < 0;
        input.left |= stepped.x < 0;
        input.right |= stepped.x > 0;
        input.accept |= pad.just_pressed(GamepadButton::South);
        input.cancel |= pad.just_pressed(GamepadButton::East);
        input.start |= pad.just_pressed(GamepadButton::Start);
    }
}

/// The d-pad and the left stick point the same way, quantised to one step per
/// axis so that either device produces the same single move.
fn direction(pad: &Gamepad) -> IVec2 {
    let axes = pad.dpad() + pad.left_stick();
    IVec2::new(quantise(axes.x), quantise(axes.y))
}

fn quantise(value: f32) -> i32 {
    if value > STICK_THRESHOLD {
        1
    } else if value < -STICK_THRESHOLD {
        -1
    } else {
        0
    }
}

fn axis_edge(previous: i32, intent: i32) -> i32 {
    if previous <= 0 && intent > 0 {
        1
    } else if previous >= 0 && intent < 0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity index")
    }

    #[test]
    fn each_axis_steps_towards_the_last_button() {
        let down = MenuInput {
            down: true,
            ..default()
        };
        assert_eq!(down.step(NavAxis::Vertical), 1);
        assert_eq!(down.step(NavAxis::Horizontal), 0);

        let left = MenuInput {
            left: true,
            ..default()
        };
        assert_eq!(left.step(NavAxis::Horizontal), -1);
        assert_eq!(left.step(NavAxis::Vertical), 0);
    }

    #[test]
    fn opposite_directions_cancel_out() {
        let both = MenuInput {
            up: true,
            down: true,
            ..default()
        };
        assert_eq!(both.step(NavAxis::Vertical), 0);
    }

    #[test]
    fn start_confirms_like_the_south_button() {
        assert!(
            MenuInput {
                start: true,
                ..default()
            }
            .accepted()
        );
        assert!(!MenuInput::default().accepted());
    }

    #[test]
    fn a_held_stick_steps_once() {
        let mut latch = StickLatch::default();
        // First sighting adopts the current direction rather than stepping.
        assert_eq!(latch.edge(pad(1), IVec2::new(0, -1)), IVec2::ZERO);
        assert_eq!(latch.edge(pad(1), IVec2::new(0, -1)), IVec2::ZERO);
        assert_eq!(latch.edge(pad(1), IVec2::ZERO), IVec2::ZERO);
        assert_eq!(latch.edge(pad(1), IVec2::new(0, -1)), IVec2::new(0, -1));
    }

    #[test]
    fn the_two_axes_latch_independently() {
        let mut latch = StickLatch::default();
        latch.edge(pad(1), IVec2::ZERO);
        assert_eq!(latch.edge(pad(1), IVec2::new(1, 0)), IVec2::new(1, 0));
        assert_eq!(latch.edge(pad(1), IVec2::new(1, 1)), IVec2::new(0, 1));
    }

    #[test]
    fn unplugged_pads_are_forgotten() {
        let mut latch = StickLatch::default();
        latch.edge(pad(1), IVec2::ZERO);
        latch.edge(pad(2), IVec2::ZERO);
        latch.forget_absent(&[pad(2)]);
        assert_eq!(latch.previous.len(), 1);
    }

    #[test]
    fn a_stick_only_counts_past_the_threshold() {
        assert_eq!(quantise(0.49), 0);
        assert_eq!(quantise(0.51), 1);
        assert_eq!(quantise(-0.51), -1);
    }
}
