//! Seat roster: which device, if any, drives each of the four carts.

use bevy::prelude::*;

/// A device that can claim a seat. Each gamepad is its own source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputSource {
    Wasd,
    Arrows,
    Pad(Entity),
}

/// Which source holds each seat. An empty seat is played by a bot.
///
/// This is the authority on seating. It is a resource so that it survives the
/// round restart, leaving the lobby, and returning to the main menu.
#[derive(Resource, Default)]
pub(crate) struct Roster {
    seats: [Option<InputSource>; 4],
}

impl Roster {
    pub(crate) fn seat(&self, id: usize) -> Option<InputSource> {
        self.seats.get(id).copied().flatten()
    }

    pub(crate) fn seat_of(&self, source: InputSource) -> Option<usize> {
        self.seats.iter().position(|seat| *seat == Some(source))
    }

    /// Claim the lowest free seat. A source that already holds one, and a full
    /// roster, both refuse.
    pub(crate) fn join(&mut self, source: InputSource) -> Option<usize> {
        if self.seat_of(source).is_some() {
            return None;
        }
        let index = self.seats.iter().position(Option::is_none)?;
        self.seats[index] = Some(source);
        Some(index)
    }

    pub(crate) fn leave(&mut self, source: InputSource) -> bool {
        match self.seat_of(source) {
            Some(index) => {
                self.seats[index] = None;
                true
            }
            None => false,
        }
    }

    pub(crate) fn humans(&self) -> usize {
        self.seats.iter().flatten().count()
    }

    /// Free the seats of gamepads that are no longer connected. Keyboard seats
    /// are never touched.
    pub(crate) fn retain_connected(&mut self, connected: &[Entity]) {
        for seat in &mut self.seats {
            if let Some(InputSource::Pad(entity)) = seat
                && !connected.contains(entity)
            {
                *seat = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(index: u32) -> InputSource {
        InputSource::Pad(Entity::from_raw_u32(index).expect("valid entity index"))
    }

    #[test]
    fn joining_fills_seats_in_order() {
        let mut roster = Roster::default();
        assert_eq!(roster.join(InputSource::Wasd), Some(0));
        assert_eq!(roster.join(InputSource::Arrows), Some(1));
        assert_eq!(roster.join(pad(7)), Some(2));
        assert_eq!(roster.humans(), 3);
    }

    #[test]
    fn a_source_cannot_hold_two_seats() {
        let mut roster = Roster::default();
        assert_eq!(roster.join(InputSource::Wasd), Some(0));
        assert_eq!(roster.join(InputSource::Wasd), None);
        assert_eq!(roster.humans(), 1);
    }

    #[test]
    fn leaving_frees_the_seat_for_the_next_joiner() {
        let mut roster = Roster::default();
        roster.join(InputSource::Wasd);
        roster.join(InputSource::Arrows);
        roster.join(pad(7));
        assert!(roster.leave(InputSource::Arrows));
        assert_eq!(roster.join(pad(9)), Some(1));
    }

    #[test]
    fn a_fifth_source_cannot_join() {
        let mut roster = Roster::default();
        roster.join(InputSource::Wasd);
        roster.join(InputSource::Arrows);
        roster.join(pad(7));
        roster.join(pad(8));
        assert_eq!(roster.join(pad(9)), None);
    }

    #[test]
    fn empty_seats_are_bots() {
        let mut roster = Roster::default();
        roster.join(InputSource::Wasd);
        assert_eq!(roster.seat(0), Some(InputSource::Wasd));
        assert_eq!(roster.seat(1), None);
        assert_eq!(roster.seat(3), None);
    }

    #[test]
    fn disconnected_pads_lose_their_seat_but_keyboards_keep_theirs() {
        let mut roster = Roster::default();
        roster.join(InputSource::Wasd);
        roster.join(pad(7));
        roster.join(pad(8));

        let still_here = Entity::from_raw_u32(7).expect("valid entity index");
        roster.retain_connected(&[still_here]);

        assert_eq!(roster.seat(0), Some(InputSource::Wasd));
        assert_eq!(roster.seat(1), Some(pad(7)));
        assert_eq!(roster.seat(2), None);
    }
}
