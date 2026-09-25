//! Main menu, pause controls, and the score and timer HUD.

mod lobby;
mod nav;
mod settings;

pub(crate) use lobby::{
    LobbyLatch, despawn_lobby, lobby_input, lobby_shortcuts, refresh_lobby, spawn_lobby,
};
pub(crate) use nav::{MenuInput, StickLatch, gather_menu_input};
pub(crate) use settings::{adjust_focused_setting, refresh_setting_values, settings_are_open};

use bevy::prelude::*;

use crate::game::{AppState, ROUND_SECONDS, Round};
use crate::players::{Player, Roster};
use crate::settings::VideoSettings;
use nav::NavAxis;
use settings::{SettingRow, SettingsUi, spawn_settings};

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
    Back,
    Exit,
    Resume,
    ReturnToMenu,
    OpenSettings,
    CloseSettings,
    Setting(SettingRow),
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
    players: Query<&Player>,
    mut hud: Single<&mut Text, With<Hud>>,
    mut shown: Local<Option<(u32, [u32; 4], bool)>>,
) {
    let remaining = (ROUND_SECONDS - round.timer.elapsed_secs()).max(0.0).ceil() as u32;
    let mut score_by_id = [0; 4];
    for player in &players {
        if let Some(score) = score_by_id.get_mut(player.id) {
            *score = player.score;
        }
    }
    // The text only changes about once a second. Rewriting it every frame
    // marks it changed, and Bevy then reshapes and lays it out again.
    let key = (remaining, score_by_id, round.finished);
    if *shown == Some(key) {
        return;
    }
    *shown = Some(key);

    let mut scores: Vec<(usize, u32)> = players.iter().map(|p| (p.id, p.score)).collect();
    scores.sort_by_key(|(id, _)| *id);
    let score_line = scores
        .iter()
        .map(|(id, score)| format!("P{}: {}", id + 1, score))
        .collect::<Vec<_>>()
        .join("   ");

    hud.0 = if round.finished {
        let winner = scores
            .iter()
            .max_by_key(|(_, score)| *score)
            .map(|(id, _)| id + 1)
            .unwrap_or(1);
        format!("TIME!  Player {winner} wins\n{score_line}\nPress R or gamepad Start to restart")
    } else {
        format!(
            "{remaining:02}s   {score_line}\nSteer at junctions  |  Fire to shoot  |  Hit opponents from behind"
        )
    };
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
            parent.spawn(menu_button("SETTINGS", MenuAction::OpenSettings, 1));
            if CAN_EXIT {
                parent.spawn(menu_button("EXIT", MenuAction::Exit, 2));
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
                    panel.spawn(menu_button("SETTINGS", MenuAction::OpenSettings, 1));
                    panel.spawn(menu_button("RETURN TO MENU", MenuAction::ReturnToMenu, 2));
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
    main_menus: Query<'w, 's, Entity, With<MainMenuUi>>,
    settings: Query<'w, 's, (Entity, &'static SettingsUi)>,
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
    mut video: ResMut<VideoSettings>,
    roster: Res<Roster>,
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
        MenuAction::OpenSettings => {
            // Only one screen's buttons may exist at a time, so the screen
            // underneath makes way and is rebuilt when settings close.
            let origin = if screens.pause_dialogs.is_empty() {
                SettingsUi::MainMenu
            } else {
                SettingsUi::Pause
            };
            for entity in screens.pause_dialogs.iter().chain(&screens.main_menus) {
                commands.entity(entity).despawn();
            }
            spawn_settings(&mut commands, origin, &video);
        }
        MenuAction::CloseSettings => close_settings(&mut commands, &screens.settings),
        MenuAction::Setting(row) => row.cycle(&mut video, 1),
    }
}

/// Close the settings screen and bring back the one it was opened from.
fn close_settings(commands: &mut Commands, open: &Query<(Entity, &SettingsUi)>) {
    for (entity, origin) in open {
        commands.entity(entity).despawn();
        match origin {
            SettingsUi::MainMenu => main_menu(commands),
            SettingsUi::Pause => spawn_pause_dialog(commands),
        }
    }
}

/// Escape and the pad's east button back out of the settings screen.
pub(crate) fn settings_shortcuts(
    input: Res<MenuInput>,
    mut commands: Commands,
    open: Query<(Entity, &SettingsUi)>,
) {
    if input.cancel {
        close_settings(&mut commands, &open);
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

pub(crate) fn despawn_main_menu(
    mut commands: Commands,
    menus: Query<Entity, Or<(With<MainMenuUi>, With<SettingsUi>)>>,
) {
    for entity in &menus {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn close_pause_dialog(
    mut commands: Commands,
    mut pause: ResMut<PauseState>,
    dialogs: Query<Entity, Or<(With<PauseDialogUi>, With<SettingsUi>)>>,
) {
    pause.0 = false;
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
}

pub(crate) fn gameplay_is_running(pause: Res<PauseState>) -> bool {
    !pause.0
}
