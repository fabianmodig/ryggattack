//! One-life elimination, first-to match scoring, and automatic round resets.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::match_settings::MatchSettings;
use crate::players::{Player, Roster};
use crate::scenery::{Forest, Transient, rebuild_forest};
use crate::tracks::{RailFollower, RailMap, reset_cart};

#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub(crate) enum AppState {
    #[default]
    MainMenu,
    Lobby,
    Playing,
}

#[derive(Resource)]
pub(crate) struct Round {
    pub(crate) finished: bool,
    pub(crate) wins: [u32; 4],
    pub(crate) winner: Option<usize>,
    pub(crate) round_winner: Option<usize>,
    pub(crate) transition: f32,
}

impl Default for Round {
    fn default() -> Self {
        Self {
            finished: false,
            wins: [0; 4],
            winner: None,
            round_winner: None,
            transition: 0.0,
        }
    }
}

impl Round {
    fn resolve(&mut self, survivors: &[usize], participants: usize, target: u32) -> Option<usize> {
        if participants < 2 || self.finished || survivors.len() > 1 {
            return None;
        }
        self.finished = true;
        self.transition = 1.5;
        self.round_winner = survivors.first().copied();
        if let Some(id) = self.round_winner {
            self.wins[id] += 1;
            if self.wins[id] >= target {
                self.winner = Some(id);
            }
        }
        self.round_winner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_to_target_stops_the_match() {
        let mut round = Round::default();
        for n in 1..=3 {
            round.finished = false;
            assert_eq!(round.resolve(&[1], 4, 3), Some(1));
            assert_eq!(round.wins[1], n);
            assert_eq!(round.winner, if n == 3 { Some(1) } else { None });
        }
    }
    #[test]
    fn all_dead_is_a_draw_not_a_point() {
        let mut round = Round::default();
        assert_eq!(round.resolve(&[], 4, 3), None);
        assert!(round.finished);
    }
    #[test]
    fn solo_practice_never_finishes() {
        let mut round = Round::default();
        for _ in 0..10 {
            assert_eq!(round.resolve(&[0], 1, 3), None);
        }
        assert!(!round.finished);
    }
    #[test]
    fn last_standing_wins_exactly_once() {
        let mut round = Round::default();
        assert_eq!(round.resolve(&[2], 4, 3), Some(2));
        assert_eq!(round.resolve(&[2], 4, 3), None);
    }
}

/// Everything a round reset touches, gathered so that both the start of a
/// game and a restart read the same one thing.
#[derive(SystemParam)]
pub(crate) struct RoundReset<'w, 's> {
    commands: Commands<'w, 's>,
    round: ResMut<'w, Round>,
    powerups: ResMut<'w, crate::powerups::Powerups>,
    roster: Res<'w, Roster>,
    inputs: Option<Res<'w, crate::input::SeatInputs>>,
    settings: Res<'w, MatchSettings>,
    rail_map: Res<'w, RailMap>,
    forest: Res<'w, Forest>,
    players: Query<
        'w,
        's,
        (
            &'static mut Player,
            &'static mut RailFollower,
            &'static mut Transform,
            &'static mut Visibility,
        ),
    >,
    transient: Query<'w, 's, Entity, With<Transient>>,
}

impl RoundReset<'_, '_> {
    /// Reset the round and every cart, and apply the current
    /// seating. This is the single point where the roster becomes gameplay
    /// state. The forest grows back too, along with clearing away missiles
    /// and wreckage.
    fn apply(&mut self, new_match: bool) {
        let wins = if new_match { [0; 4] } else { self.round.wins };
        *self.round = Round { wins, ..default() };
        self.powerups.reset();
        let active = self.inputs.as_ref().map_or_else(
            || self.roster.active_seats(self.settings.bots),
            |inputs| {
                inputs.occupied.unwrap_or_else(|| {
                    self.roster
                        .active_seats_with_external(self.settings.bots, inputs.external)
                })
            },
        );
        for (mut player, mut rail, mut transform, mut visibility) in self.players.iter_mut() {
            player.active = active[player.id];
            player.alive = player.active;
            *visibility = if player.active {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            player.score = 0;
            player.shot_cooldown = 0.0;
            player.wants_to_fire = false;
            player.input = self.roster.seat(player.id);
            reset_cart(&self.rail_map, player.id, &mut rail, &mut transform);
        }
        rebuild_forest(&mut self.commands, &self.forest, &self.transient);
    }
}

#[cfg(test)]
fn next_round(mut reset: RoundReset) {
    reset.apply(false);
}

#[cfg(test)]
mod reset_tests {
    use super::*;
    #[test]
    fn mouse_restart_clears_match_wins() {
        let mut app = App::new();
        let map = RailMap::random();
        let forest = Forest::new(&mut Assets::default(), &mut Assets::default(), &map);
        app.insert_resource(map)
            .init_resource::<crate::powerups::Powerups>()
            .insert_resource(forest)
            .insert_resource(Roster::default())
            .insert_resource(crate::match_settings::MatchSettings::default())
            .insert_resource(ButtonInput::<KeyCode>::default());
        app.insert_resource(Round {
            wins: [3, 0, 0, 0],
            winner: Some(0),
            finished: true,
            ..default()
        });
        app.world_mut()
            .spawn((crate::ui::RestartButton, Interaction::Pressed));
        app.add_systems(Update, restart_round);
        app.update();
        assert_eq!(app.world().resource::<Round>().wins, [0; 4]);
        assert!(!app.world().resource::<Round>().finished);
    }
    #[test]
    fn elimination_automatically_starts_next_round() {
        let mut app = App::new();
        let map = RailMap::random();
        let forest = Forest::new(&mut Assets::default(), &mut Assets::default(), &map);
        app.init_resource::<crate::powerups::Powerups>()
            .insert_resource(map)
            .insert_resource(forest);
        let mut drops = crate::powerups::Powerups::default();
        drops.until_drop = 2.0;
        drops.buffs[0].grant(crate::powerups::Kind::Speed);
        app.insert_resource(drops);
        let drop = app
            .world_mut()
            .spawn((
                crate::powerups::Pickup(crate::powerups::Kind::AllHits),
                Transient,
            ))
            .id();
        let mut roster = Roster::default();
        roster.join(crate::players::InputSource::Wasd);
        roster.join(crate::players::InputSource::Arrows);
        app.insert_resource(roster)
            .insert_resource(crate::match_settings::MatchSettings {
                bots: 0,
                ..default()
            });
        app.insert_resource(Round::default())
            .insert_resource(Time::<()>::default());
        for id in 0..2 {
            app.world_mut().spawn((
                Player {
                    id,
                    active: true,
                    alive: id == 0,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: None,
                },
                RailFollower {
                    path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                    progress: 0.0,
                    requested_turn: Vec3::ZERO,
                    collision_cooldown: 0.0,
                },
                Transform::default(),
                Visibility::Inherited,
            ));
        }
        app.add_systems(Update, tick_round);
        app.update();
        assert_eq!(app.world().resource::<Round>().wins, [1, 0, 0, 0]);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(2));
        app.update();
        assert!(!app.world().resource::<Round>().finished);
        assert_eq!(app.world().resource::<Round>().wins, [1, 0, 0, 0]);
        assert_eq!(
            app.world()
                .resource::<crate::powerups::Powerups>()
                .until_drop,
            30.0
        );
        assert_eq!(
            app.world().resource::<crate::powerups::Powerups>().buffs[0].speed,
            0.0
        );
        assert!(app.world().get_entity(drop).is_err());
        assert!(
            app.world_mut()
                .query::<&Player>()
                .iter(app.world())
                .all(|p| p.alive)
        );
    }
    #[test]
    fn next_round_restores_seating_and_preserves_wins() {
        let mut app = App::new();
        let map = RailMap::random();
        let forest = Forest::new(&mut Assets::default(), &mut Assets::default(), &map);
        app.init_resource::<crate::powerups::Powerups>()
            .insert_resource(map)
            .insert_resource(forest);
        let mut drops = crate::powerups::Powerups::default();
        drops.until_drop = 2.0;
        drops.buffs[0].grant(crate::powerups::Kind::Speed);
        app.insert_resource(drops);
        let drop = app
            .world_mut()
            .spawn((
                crate::powerups::Pickup(crate::powerups::Kind::AllHits),
                Transient,
            ))
            .id();
        let mut roster = Roster::default();
        roster.join(crate::players::InputSource::Wasd);
        app.insert_resource(roster);
        app.insert_resource(crate::match_settings::MatchSettings {
            bots: 0,
            ..default()
        });
        let mut round = Round::default();
        round.wins[0] = 2;
        app.insert_resource(round);
        let cart = app
            .world_mut()
            .spawn((
                Player {
                    id: 0,
                    active: false,
                    alive: false,
                    score: 0,
                    shot_cooldown: 1.0,
                    wants_to_fire: true,
                    input: None,
                },
                RailFollower {
                    path: crate::tracks::RailPath::Edge { from: 0, to: 1 },
                    progress: 0.0,
                    requested_turn: Vec3::X,
                    collision_cooldown: 1.0,
                },
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        let missile = app.world_mut().spawn(Transient).id();
        app.add_systems(Update, next_round);
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::powerups::Powerups>()
                .until_drop,
            30.0
        );
        assert_eq!(
            app.world().resource::<crate::powerups::Powerups>().buffs[0].speed,
            0.0
        );
        assert!(app.world().get_entity(drop).is_err());
        assert_eq!(app.world().resource::<Round>().wins[0], 2);
        let player = app.world().get::<Player>(cart).unwrap();
        assert!(player.active && player.alive);
        assert!(!player.wants_to_fire);
        assert_eq!(
            *app.world().get::<Visibility>(cart).unwrap(),
            Visibility::Inherited
        );
        assert!(app.world().get_entity(missile).is_err());
    }
}

pub(crate) fn start_new_game(mut reset: RoundReset) {
    reset.apply(true);
}

pub(crate) fn tick_round(time: Res<Time>, mut reset: RoundReset) {
    if reset.round.winner.is_some() {
        return;
    }
    if reset.round.finished {
        reset.round.transition -= time.delta_secs();
        if reset.round.transition <= 0.0 {
            reset.apply(false);
        }
        return;
    }
    let participants = reset.players.iter().filter(|(p, _, _, _)| p.active).count();
    let survivors: Vec<usize> = reset
        .players
        .iter()
        .filter(|(p, _, _, _)| p.active && p.alive)
        .map(|(p, _, _, _)| p.id)
        .collect();
    let target = reset.settings.wins_target;
    reset.round.resolve(&survivors, participants, target);
}

pub(crate) fn round_is_live(round: Res<Round>) -> bool {
    !round.finished
}

pub(crate) fn restart_round(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    buttons: Query<&Interaction, (With<crate::ui::RestartButton>, Changed<Interaction>)>,
    mut reset: RoundReset,
) {
    // Start rather than a face button, so that a pad still holding fire as the
    // match ends does not restart the moment it is tapped again.
    let requested = buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
        || keys.just_pressed(KeyCode::KeyR)
        || pads
            .iter()
            .any(|pad| pad.just_pressed(GamepadButton::Start));
    if reset.round.winner.is_none() || !requested {
        return;
    }

    reset.apply(true);
}
