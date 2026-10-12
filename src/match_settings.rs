//! Match configuration, independent of adaptive video quality.
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Arena {
    #[default]
    Forest,
    Volcano,
}

#[derive(Resource)]
pub(crate) struct MatchSettings {
    pub(crate) bots: usize,
    pub(crate) wins_target: u32,
    pub(crate) arena: Arena,
}
impl Default for MatchSettings {
    fn default() -> Self {
        Self {
            bots: 3,
            wins_target: 3,
            arena: Arena::Forest,
        }
    }
}

impl MatchSettings {
    pub(crate) fn cycle_bots(&mut self, humans: usize) {
        let maximum = 4usize.saturating_sub(humans);
        self.bots = (self.bots.min(maximum) + 1) % (maximum + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bot_selector_includes_zero_and_caps_at_empty_seats() {
        let mut settings = MatchSettings::default();
        settings.cycle_bots(1);
        assert_eq!(settings.bots, 0);
        settings.cycle_bots(1);
        assert_eq!(settings.bots, 1);
        settings.cycle_bots(3);
        assert_eq!(settings.bots, 0);
        settings.cycle_bots(4);
        assert_eq!(settings.bots, 0);
    }
    #[test]
    fn default_match_is_first_to_three_with_three_bots() {
        let settings = MatchSettings::default();
        assert_eq!(settings.bots, 3);
        assert_eq!(settings.wins_target, 3);
        assert_eq!(settings.arena, Arena::Forest);
    }
}
