//! The join lobby: press up on a device to claim a seat, down to leave.

use bevy::prelude::*;

use super::{MenuAction, MenuInput, menu_button};
use crate::game::AppState;
use crate::players::{InputSource, PLAYER_COLORS, Roster, pad_axes};

/// How far a stick must travel before it counts as a join or leave.
const JOIN_THRESHOLD: f32 = 0.5;

#[derive(Component)]
pub(crate) struct LobbyUi;

#[derive(Component)]
pub(crate) struct SeatCard(usize);

#[derive(Component)]
pub(crate) struct SeatLabel(usize);

#[derive(Component)]
pub(crate) struct LobbyHint;

/// Last frame's vertical intent per device. Sticks are analog and produce no
/// press edge of their own, so joins are edge-detected here instead.
#[derive(Resource, Default)]
pub(crate) struct LobbyLatch {
    previous: Vec<(InputSource, i8)>,
}

impl LobbyLatch {
    /// Record `intent` and report whether it just crossed into up or down.
    fn edge(&mut self, source: InputSource, intent: i8) -> i8 {
        let slot = self.previous.iter_mut().find(|(known, _)| *known == source);
        match slot {
            Some((_, previous)) => {
                let crossed = if *previous <= 0 && intent > 0 {
                    1
                } else if *previous >= 0 && intent < 0 {
                    -1
                } else {
                    0
                };
                *previous = intent;
                crossed
            }
            None => {
                // First sighting: adopt the current intent so a key still held
                // from the previous screen does not read as a fresh press.
                self.previous.push((source, intent));
                0
            }
        }
    }

    fn forget_absent(&mut self, present: &[InputSource]) {
        self.previous.retain(|(source, _)| present.contains(source));
    }
}

pub(crate) fn spawn_lobby(mut commands: Commands, mut latch: ResMut<LobbyLatch>) {
    latch.previous.clear();

    commands
        .spawn((
            LobbyUi,
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(22),
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.035, 0.065)),
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("CHOOSE YOUR CART"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.78, 0.24)),
                TextShadow::default(),
            ));
            parent.spawn((
                Text::new("UP to join   |   DOWN to leave   |   choose bots, win target and arena"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.78, 0.90)),
            ));
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(18),
                    ..default()
                })
                .with_children(|row| {
                    for id in 0..4 {
                        row.spawn(seat_card(id));
                    }
                });
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(18),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(menu_button("BOTS: 3", MenuAction::CycleBots, 1));
                    row.spawn(menu_button("FIRST TO: 3", MenuAction::CycleTarget, 2));
                    row.spawn(menu_button("ARENA: FOREST", MenuAction::CycleArena, 3));
                });
            // Side by side, because up and down claim and release seats here
            // and the highlight steps sideways instead.
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(18),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(menu_button("START MATCH", MenuAction::Launch, 0));
                    row.spawn(menu_button("BACK", MenuAction::Back, 4));
                });
            parent.spawn((
                LobbyHint,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.62, 0.74)),
            ));
        });
}

fn seat_card(id: usize) -> impl Bundle {
    (
        SeatCard(id),
        Node {
            width: px(190),
            height: px(150),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: px(10),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(14)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.10, 0.16)),
        BorderColor::all(PLAYER_COLORS[id]),
        children![
            (
                Text::new(format!("P{}", id + 1)),
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
                TextColor(PLAYER_COLORS[id]),
            ),
            (
                SeatLabel(id),
                Text::new("BOT"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.50, 0.60)),
            ),
        ],
    )
}

/// Claim and release seats, and drop pads that were unplugged.
pub(crate) fn lobby_input(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<(Entity, &Gamepad)>,
    mut roster: ResMut<Roster>,
    mut latch: ResMut<LobbyLatch>,
) {
    let connected: Vec<Entity> = pads.iter().map(|(entity, _)| entity).collect();
    roster.retain_connected(&connected);

    let mut sources = vec![
        (
            InputSource::Wasd,
            key_intent(&keys, KeyCode::KeyW, KeyCode::KeyS),
        ),
        (
            InputSource::Arrows,
            key_intent(&keys, KeyCode::ArrowUp, KeyCode::ArrowDown),
        ),
    ];
    for (entity, pad) in &pads {
        sources.push((InputSource::Pad(entity), pad_intent(pad)));
    }

    let present: Vec<InputSource> = sources.iter().map(|(source, _)| *source).collect();
    latch.forget_absent(&present);

    for (source, intent) in sources {
        match latch.edge(source, intent) {
            1 => {
                roster.join(source);
            }
            -1 => {
                roster.leave(source);
            }
            _ => {}
        }
    }
}

/// Starting the round goes through the highlighted button, so only backing out
/// is left to a shortcut here.
pub(crate) fn lobby_shortcuts(input: Res<MenuInput>, mut next_state: ResMut<NextState<AppState>>) {
    if input.cancel {
        next_state.set(AppState::MainMenu);
    }
}

pub(crate) fn refresh_lobby(
    roster: Res<Roster>,
    mut settings: ResMut<crate::match_settings::MatchSettings>,
    pads: Query<(Entity, &Gamepad, Option<&Name>)>,
    mut labels: Query<(&SeatLabel, &mut Text, &mut TextColor), Without<LobbyHint>>,
    mut cards: Query<(&SeatCard, &mut BackgroundColor)>,
    mut hints: Query<&mut Text, With<LobbyHint>>,
) {
    settings.bots = settings.bots.min(4 - roster.humans());
    let active = roster.active_seats(settings.bots);
    for (label, mut text, mut color) in &mut labels {
        let (caption, tint) = match roster.seat(label.0) {
            None => (
                (if active[label.0] { "BOT" } else { "EMPTY" }).to_string(),
                Color::srgb(0.45, 0.50, 0.60),
            ),
            Some(InputSource::Wasd) => ("WASD".to_string(), Color::WHITE),
            Some(InputSource::Arrows) => ("ARROWS".to_string(), Color::WHITE),
            Some(InputSource::Pad(entity)) => (pad_name(&pads, entity), Color::WHITE),
        };
        text.0 = caption;
        color.0 = tint;
    }

    for (card, mut background) in &mut cards {
        background.0 = match roster.seat(card.0) {
            Some(_) => PLAYER_COLORS[card.0].with_alpha(0.18),
            None => Color::srgb(0.08, 0.10, 0.16),
        };
    }

    for mut hint in &mut hints {
        hint.0 = if roster.humans() == 0 {
            "Press UP on a keyboard or gamepad to take a seat".into()
        } else {
            "Left/Right: choose   |   Enter, A, or Start: select   |   Escape or B: back".into()
        };
    }
}

pub(crate) fn refresh_setting_labels(
    settings: Res<crate::match_settings::MatchSettings>,
    buttons: Query<(&MenuAction, &Children)>,
    mut texts: Query<&mut Text>,
) {
    for (action, children) in &buttons {
        let caption = match action {
            MenuAction::CycleBots => format!("BOTS: {}", settings.bots),
            MenuAction::CycleTarget => format!("FIRST TO: {}", settings.wins_target),
            MenuAction::CycleArena => format!(
                "ARENA: {}",
                match settings.arena {
                    crate::match_settings::Arena::Forest => "FOREST",
                    crate::match_settings::Arena::Volcano => "VOLCANO",
                }
            ),
            _ => continue,
        };
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child)
                && text.0 != caption
            {
                text.0 = caption.clone();
            }
        }
    }
}

pub(crate) fn despawn_lobby(mut commands: Commands, lobbies: Query<Entity, With<LobbyUi>>) {
    for entity in &lobbies {
        commands.entity(entity).despawn();
    }
}

fn pad_name(pads: &Query<(Entity, &Gamepad, Option<&Name>)>, entity: Entity) -> String {
    let Ok((_, _, name)) = pads.get(entity) else {
        return "GAMEPAD".into();
    };
    let Some(name) = name else {
        return "GAMEPAD".into();
    };
    name.as_str()
        .chars()
        .take(14)
        .collect::<String>()
        .to_uppercase()
}

fn key_intent(keys: &ButtonInput<KeyCode>, up: KeyCode, down: KeyCode) -> i8 {
    match (keys.pressed(up), keys.pressed(down)) {
        (true, false) => 1,
        (false, true) => -1,
        _ => 0,
    }
}

fn pad_intent(pad: &Gamepad) -> i8 {
    let up = pad_axes(pad).y;
    if up > JOIN_THRESHOLD {
        1
    } else if up < -JOIN_THRESHOLD {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_labels_follow_selected_values() {
        let mut app = App::new();
        app.init_resource::<LobbyLatch>();
        app.insert_resource(crate::match_settings::MatchSettings {
            bots: 0,
            wins_target: 5,
            arena: crate::match_settings::Arena::Volcano,
        });
        app.add_systems(Startup, spawn_lobby)
            .add_systems(Update, refresh_setting_labels);
        app.update();
        let captions: Vec<String> = app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .map(|t| t.0.clone())
            .collect();
        for wanted in ["BOTS: 0", "FIRST TO: 5", "ARENA: VOLCANO"] {
            assert!(captions.contains(&wanted.to_string()), "missing {wanted}");
        }
    }
    #[test]
    fn unused_seat_is_labelled_empty_when_bots_are_zero() {
        let mut app = App::new();
        let mut roster = Roster::default();
        roster.join(InputSource::Wasd);
        app.insert_resource(roster)
            .insert_resource(crate::match_settings::MatchSettings {
                bots: 0,
                ..default()
            });
        let label = app
            .world_mut()
            .spawn((SeatLabel(1), Text::new("BOT"), TextColor(Color::WHITE)))
            .id();
        app.add_systems(Update, refresh_lobby);
        app.update();
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "EMPTY");
    }
    #[test]
    fn lobby_exposes_all_match_settings() {
        let mut app = App::new();
        app.init_resource::<LobbyLatch>();
        app.add_systems(Startup, spawn_lobby);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&MenuAction>()
                .iter(app.world())
                .count(),
            5
        );
    }
    #[test]
    fn a_held_direction_only_fires_once() {
        let mut latch = LobbyLatch::default();
        // First sighting adopts the current intent rather than reading as a press.
        assert_eq!(latch.edge(InputSource::Wasd, 1), 0);
        assert_eq!(latch.edge(InputSource::Wasd, 1), 0);
        assert_eq!(latch.edge(InputSource::Wasd, 0), 0);
        assert_eq!(latch.edge(InputSource::Wasd, 1), 1);
    }

    #[test]
    fn down_reports_a_leave_edge() {
        let mut latch = LobbyLatch::default();
        latch.edge(InputSource::Arrows, 0);
        assert_eq!(latch.edge(InputSource::Arrows, -1), -1);
        assert_eq!(latch.edge(InputSource::Arrows, -1), 0);
    }

    #[test]
    fn unplugged_devices_are_forgotten() {
        let mut latch = LobbyLatch::default();
        latch.edge(InputSource::Wasd, 0);
        latch.edge(InputSource::Arrows, 0);
        latch.forget_absent(&[InputSource::Wasd]);
        assert_eq!(latch.previous.len(), 1);
    }
}
