//! Main menu, match settings, pause controls, and the round-wins HUD.

mod lobby;
mod nav;

pub(crate) use lobby::{
    LobbyLatch, despawn_lobby, lobby_input, lobby_shortcuts, refresh_lobby, refresh_setting_labels,
    spawn_lobby,
};
pub(crate) use nav::{MenuInput, StickLatch, gather_menu_input};

use bevy::prelude::*;

use crate::game::{AppState, Round};
use crate::players::{Player, Roster};
use nav::NavAxis;

/// The browser tab owns the page, so the web build cannot close itself and
/// leaves out the affordances that would only freeze the canvas.
const CAN_EXIT: bool = !cfg!(target_arch = "wasm32");

#[derive(Resource, Default)]
pub(crate) struct PauseState(bool);

#[derive(Component)]
pub(crate) struct Hud;

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct PauseDialogUi;

/// A button the menu highlight can land on, ordered first to last within its
/// screen. Only one screen's buttons exist at a time, so the order is enough
/// to step through them.
#[derive(Component, Clone, Copy)]
pub(crate) struct MenuItem {
    order: usize,
}

/// The button that Enter and the pad's accept button act on.
#[derive(Resource, Default)]
pub(crate) struct MenuFocus(Option<Entity>);

#[derive(Component, Clone, Copy)]
pub(crate) enum MenuAction {
    Start,
    Launch,
    CycleBots,
    CycleTarget,
    CycleArena,
    Back,
    Exit,
    Resume,
    ReturnToMenu,
}

pub(crate) fn spawn_hud(commands: &mut Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(25.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            left: px(22),
            ..default()
        },
    ));
}

pub(crate) fn update_hud(
    round: Res<Round>,
    settings: Res<crate::match_settings::MatchSettings>,
    powerups: Option<Res<crate::powerups::Powerups>>,
    players: Query<&Player>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let mut seats: Vec<&Player> = players.iter().filter(|p| p.active).collect();
    seats.sort_by_key(|p| p.id);
    let scores = seats
        .iter()
        .map(|p| {
            let buffs = powerups
                .as_ref()
                .map_or(default(), |powerups| powerups.buffs[p.id]);
            let mut effects = String::new();
            if p.alive && buffs.speed > 0.0 {
                effects.push_str(&format!(" [SPEED +25% {:.0}s]", buffs.speed.ceil()));
            }
            if p.alive && buffs.all_hits > 0.0 {
                effects.push_str(&format!(" [ALL HITS {:.0}s]", buffs.all_hits.ceil()));
            }
            format!(
                "P{}: {}{}{effects}",
                p.id + 1,
                round.wins[p.id],
                if p.alive { "" } else { " (OUT)" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let status = if let Some(winner) = round.winner {
        format!(
            "MATCH OVER - Player {} wins!\nR, gamepad Start, or click RESTART MATCH",
            winner + 1
        )
    } else if round.finished {
        match round.round_winner {
            Some(id) => format!("Player {} wins the round! Next round...", id + 1),
            None => "DRAW - no point. Next round...".into(),
        }
    } else if seats.len() == 1 {
        "SOLO PRACTICE - no scoring".into()
    } else {
        "ONE LIFE - rear hits eliminate (ALL HITS: any side). Last standing wins.".into()
    };
    let text = format!(
        "FIRST TO {}   {scores}\n{status}\nRail drops: yellow = SPEED +25%, pink = ALL HITS (10s)",
        settings.wins_target
    );
    if hud.0 != text {
        hud.0 = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_shows_coexisting_buffs_and_hides_expired_effects() {
        let mut app = App::new();
        let mut powerups = crate::powerups::Powerups::default();
        powerups.buffs[0].grant(crate::powerups::Kind::Speed);
        powerups.buffs[0].grant(crate::powerups::Kind::AllHits);
        app.insert_resource(powerups)
            .insert_resource(Round::default())
            .insert_resource(crate::match_settings::MatchSettings::default());
        app.world_mut().spawn(Player {
            id: 0,
            active: true,
            alive: true,
            score: 0,
            shot_cooldown: 0.0,
            wants_to_fire: false,
            input: None,
        });
        let hud = app.world_mut().spawn((Hud, Text::new(""))).id();
        app.add_systems(Update, update_hud);
        app.update();
        let text = &app.world().get::<Text>(hud).unwrap().0;
        assert!(text.contains("SPEED +25% 10s"));
        assert!(text.contains("ALL HITS 10s"));
        app.world_mut()
            .resource_mut::<crate::powerups::Powerups>()
            .buffs[0] = default();
        app.update();
        let text = &app.world().get::<Text>(hud).unwrap().0;
        assert!(!text.contains("[SPEED +25%"));
        assert!(!text.contains("ALL HITS 10s"));
    }
    fn action_app(action: MenuAction, mouse: bool, start: bool) -> App {
        let mut app = App::new();
        let button = app
            .world_mut()
            .spawn((
                Button,
                action,
                if mouse {
                    Interaction::Pressed
                } else {
                    Interaction::None
                },
            ))
            .id();
        let mut roster = Roster::default();
        roster.join(crate::players::InputSource::Wasd);
        app.insert_resource(roster)
            .insert_resource(crate::match_settings::MatchSettings::default());
        app.insert_resource(MenuFocus(Some(button)))
            .insert_resource(MenuInput {
                accept: !mouse && !start,
                start,
                ..default()
            });
        app.init_resource::<PauseState>()
            .init_resource::<NextState<AppState>>();
        app.add_systems(Update, handle_menu_actions);
        app
    }
    #[test]
    fn match_over_exposes_a_mouse_restart_control() {
        let mut app = App::new();
        app.insert_resource(Round {
            winner: Some(1),
            finished: true,
            ..default()
        });
        app.add_systems(Update, sync_match_over_ui);
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<RestartButton>>()
                .iter(app.world())
                .count(),
            1
        );
        app.world_mut().resource_mut::<Round>().winner = None;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<RestartButton>>()
                .iter(app.world())
                .count(),
            0
        );
    }
    #[test]
    fn arena_button_toggles_forest_and_volcano() {
        let mut app = action_app(MenuAction::CycleArena, true, false);
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::match_settings::MatchSettings>()
                .arena,
            crate::match_settings::Arena::Volcano
        );
        app.world_mut().resource_mut::<MenuInput>().accept = true;
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::match_settings::MatchSettings>()
                .arena,
            crate::match_settings::Arena::Forest
        );
    }
    #[test]
    fn target_button_cycles_first_to_one_three_five_seven() {
        let mut app = action_app(MenuAction::CycleTarget, false, false);
        for target in [5, 7, 1, 3] {
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::match_settings::MatchSettings>()
                    .wins_target,
                target
            );
        }
    }
    #[test]
    fn bot_button_works_with_mouse_keyboard_and_gamepad() {
        for (mouse, start) in [(true, false), (false, false), (false, true)] {
            let mut app = action_app(MenuAction::CycleBots, mouse, start);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<crate::match_settings::MatchSettings>()
                    .bots,
                0
            );
        }
    }
    #[test]
    fn hud_status_text_is_ascii_in_every_round_state() {
        for (seats, finished, round_winner, winner) in [
            (1, false, None, None),
            (2, false, None, None),
            (2, true, Some(0), None),
            (2, true, None, None),
            (2, true, Some(0), Some(0)),
        ] {
            let mut app = App::new();
            app.insert_resource(Round {
                finished,
                round_winner,
                winner,
                ..default()
            })
            .insert_resource(crate::match_settings::MatchSettings::default());
            for id in 0..seats {
                app.world_mut().spawn(Player {
                    id,
                    active: true,
                    alive: true,
                    score: 0,
                    shot_cooldown: 0.0,
                    wants_to_fire: false,
                    input: None,
                });
            }
            let hud = app.world_mut().spawn((Hud, Text::new(""))).id();
            app.add_systems(Update, update_hud);
            app.update();
            let text = &app.world().get::<Text>(hud).unwrap().0;
            assert!(text.is_ascii(), "unsupported HUD glyph: {text}");
        }
    }
    #[test]
    fn hud_shows_round_wins_instead_of_a_timer() {
        let mut app = App::new();
        app.insert_resource(Round::default())
            .insert_resource(crate::match_settings::MatchSettings::default());
        let hud = app.world_mut().spawn((Hud, Text::new(""))).id();
        app.add_systems(Update, update_hud);
        app.update();
        assert!(
            app.world()
                .get::<Text>(hud)
                .unwrap()
                .0
                .contains("FIRST TO 3")
        );
    }
}

#[derive(Component)]
pub(crate) struct RestartButton;

pub(crate) fn sync_match_over_ui(
    mut commands: Commands,
    round: Res<Round>,
    buttons: Query<Entity, With<RestartButton>>,
) {
    if round.winner.is_some() && buttons.is_empty() {
        commands.spawn((
            RestartButton,
            Button,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(30),
                left: percent(38),
                padding: UiRect::all(px(18)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.22, 0.34, 0.54)),
            GlobalZIndex(90),
            children![(
                Text::new("RESTART MATCH"),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::WHITE)
            )],
        ));
    } else if round.winner.is_none() {
        for entity in &buttons {
            commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn despawn_match_over_ui(
    mut commands: Commands,
    buttons: Query<Entity, With<RestartButton>>,
) {
    for entity in &buttons {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn spawn_main_menu(mut commands: Commands) {
    main_menu(&mut commands);
}

fn main_menu(commands: &mut Commands) {
    commands
        .spawn((
            MainMenuUi,
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
                Text::new("RYGGATTACK"),
                TextFont {
                    font_size: FontSize::Px(68.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.78, 0.24)),
                TextShadow::default(),
            ));
            parent.spawn((
                Text::new("A railway battle"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::srgb(0.72, 0.78, 0.90)),
            ));
            parent.spawn(menu_button("START", MenuAction::Start, 0));
            if CAN_EXIT {
                parent.spawn(menu_button("EXIT", MenuAction::Exit, 1));
            }
            parent.spawn((
                Text::new(if CAN_EXIT {
                    "Up/Down or D-pad: choose   |   Enter or A: select   |   Escape or B: exit"
                } else {
                    "Up/Down or D-pad: choose   |   Enter or A: select"
                }),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.62, 0.74)),
            ));
        });
}

fn spawn_pause_dialog(commands: &mut Commands) {
    commands
        .spawn((
            PauseDialogUi,
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.015, 0.03, 0.78)),
            GlobalZIndex(110),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(420),
                        padding: UiRect::all(px(38)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(20),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.07, 0.09, 0.15)),
                    BorderColor::all(Color::srgb(0.35, 0.48, 0.68)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("PAUSED"),
                        TextFont {
                            font_size: FontSize::Px(44.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    panel.spawn(menu_button("RESUME", MenuAction::Resume, 0));
                    panel.spawn(menu_button("RETURN TO MENU", MenuAction::ReturnToMenu, 1));
                    panel.spawn((
                        Text::new("Up/Down or D-pad: choose   |   Escape or B: resume"),
                        TextFont {
                            font_size: FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.58, 0.65, 0.76)),
                    ));
                });
        });
}

pub(super) fn menu_button(label: &'static str, action: MenuAction, order: usize) -> impl Bundle {
    (
        Button,
        action,
        MenuItem { order },
        Node {
            width: px(290),
            height: px(62),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.13, 0.18, 0.29)),
        BorderColor::all(Color::srgb(0.33, 0.48, 0.72)),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(27.0),
                ..default()
            },
            TextColor(Color::WHITE),
        )],
    )
}

/// Move the highlight. The mouse and the arrow keys share one highlight, so
/// hovering a button moves it there and the two can never disagree.
pub(crate) fn navigate_menu(
    input: Res<MenuInput>,
    state: Res<State<AppState>>,
    mut focus: ResMut<MenuFocus>,
    items: Query<(Entity, &MenuItem)>,
    pointed_at: Query<(Entity, &Interaction), (Changed<Interaction>, With<MenuItem>)>,
) {
    let mut ordered: Vec<(Entity, usize)> = items
        .iter()
        .map(|(entity, item)| (entity, item.order))
        .collect();
    ordered.sort_by_key(|(_, order)| *order);
    if ordered.is_empty() {
        focus.0 = None;
        return;
    }

    for (entity, interaction) in &pointed_at {
        if *interaction != Interaction::None {
            focus.0 = Some(entity);
        }
    }

    let known = focus
        .0
        .and_then(|entity| ordered.iter().position(|(button, _)| *button == entity));
    let index = match known {
        Some(index) => {
            let step = input.step(nav_axis(state.get()));
            (index as i32 + step).rem_euclid(ordered.len() as i32) as usize
        }
        // A screen that just opened, so settle on its first button and let the
        // next press step from there.
        None => 0,
    };
    focus.0 = Some(ordered[index].0);
}

/// The lobby keeps up and down for claiming seats, so its buttons sit in a row
/// and are stepped through sideways instead.
fn nav_axis(state: &AppState) -> NavAxis {
    match state {
        AppState::Lobby => NavAxis::Horizontal,
        _ => NavAxis::Vertical,
    }
}

pub(crate) fn update_menu_buttons(
    focus: Res<MenuFocus>,
    mut buttons: Query<
        (Entity, &Interaction, &mut BackgroundColor, &mut BorderColor),
        With<MenuItem>,
    >,
) {
    for (entity, interaction, mut background, mut border) in &mut buttons {
        let focused = focus.0 == Some(entity);
        background.0 = match (interaction, focused) {
            (Interaction::Pressed, _) => Color::srgb(0.90, 0.55, 0.16),
            (_, true) => Color::srgb(0.22, 0.34, 0.54),
            _ => Color::srgb(0.13, 0.18, 0.29),
        };
        *border = BorderColor::all(if focused {
            Color::srgb(0.95, 0.78, 0.24)
        } else {
            Color::srgb(0.33, 0.48, 0.72)
        });
    }
}

/// The screens a menu action opens or closes, gathered to keep the action
/// handler's parameter list readable.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Screens<'w, 's> {
    pause_dialogs: Query<'w, 's, Entity, With<PauseDialogUi>>,
}

/// Run the button the mouse clicked, or the one the highlight sits on when a
/// device confirms. Both routes end in the same match, so a menu never gains a
/// mouse-only option.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_menu_actions(
    mut commands: Commands,
    input: Res<MenuInput>,
    focus: Res<MenuFocus>,
    actions: Query<&MenuAction>,
    buttons: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    screens: Screens,
    mut pause: ResMut<PauseState>,
    roster: Res<Roster>,
    mut settings: ResMut<crate::match_settings::MatchSettings>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let clicked = buttons
        .iter()
        .find(|(interaction, _)| **interaction == Interaction::Pressed)
        .map(|(_, action)| *action);
    let confirmed = || {
        focus
            .0
            .filter(|_| input.accepted())
            .and_then(|entity| actions.get(entity).ok())
            .copied()
    };
    let Some(action) = clicked.or_else(confirmed) else {
        return;
    };

    match action {
        MenuAction::CycleBots => settings.cycle_bots(roster.humans()),
        MenuAction::CycleTarget => {
            settings.wins_target = match settings.wins_target {
                1 => 3,
                3 => 5,
                5 => 7,
                _ => 1,
            }
        }
        MenuAction::CycleArena => {
            settings.arena = match settings.arena {
                crate::match_settings::Arena::Forest => crate::match_settings::Arena::Volcano,
                crate::match_settings::Arena::Volcano => crate::match_settings::Arena::Forest,
            }
        }
        MenuAction::Start => next_state.set(AppState::Lobby),
        MenuAction::Launch => {
            if roster.humans() > 0 {
                next_state.set(AppState::Playing);
            }
        }
        MenuAction::Back => next_state.set(AppState::MainMenu),
        MenuAction::Exit => {
            commands.write_message(AppExit::Success);
        }
        MenuAction::Resume => {
            pause.0 = false;
            for entity in &screens.pause_dialogs {
                commands.entity(entity).despawn();
            }
        }
        MenuAction::ReturnToMenu => next_state.set(AppState::MainMenu),
    }
}

/// Starting the game goes through the highlighted button, so only leaving is
/// left to a shortcut here.
pub(crate) fn main_menu_shortcuts(input: Res<MenuInput>, mut commands: Commands) {
    if CAN_EXIT && input.cancel {
        commands.write_message(AppExit::Success);
    }
}

/// Escape and the pad's east button both open and close the dialog; Start only
/// opens it, because an open dialog already treats Start as a confirmation of
/// the highlighted button and would otherwise resume and re-open in one frame.
/// Start is also left alone once the clock has run out, where it restarts the
/// round instead.
pub(crate) fn toggle_pause_dialog(
    input: Res<MenuInput>,
    round: Res<Round>,
    mut commands: Commands,
    mut pause: ResMut<PauseState>,
    dialogs: Query<Entity, With<PauseDialogUi>>,
) {
    let requested = if pause.0 || round.finished {
        input.cancel
    } else {
        input.cancel || input.start
    };
    if !requested {
        return;
    }

    pause.0 = !pause.0;
    if pause.0 {
        spawn_pause_dialog(&mut commands);
    } else {
        for entity in &dialogs {
            commands.entity(entity).despawn();
        }
    }
}

pub(crate) fn despawn_main_menu(mut commands: Commands, menus: Query<Entity, With<MainMenuUi>>) {
    for entity in &menus {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn close_pause_dialog(
    mut commands: Commands,
    mut pause: ResMut<PauseState>,
    dialogs: Query<Entity, With<PauseDialogUi>>,
) {
    pause.0 = false;
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn gameplay_is_running(pause: Res<PauseState>) -> bool {
    !pause.0
}
