//! Main menu, pause controls, and the score and timer HUD.

mod lobby;

pub(crate) use lobby::{
    LobbyLatch, despawn_lobby, lobby_input, lobby_keyboard, refresh_lobby, spawn_lobby,
};

use bevy::prelude::*;

use crate::game::{AppState, ROUND_SECONDS, Round};
use crate::players::{Player, Roster};

#[derive(Resource, Default)]
pub(crate) struct PauseState(bool);

#[derive(Component)]
pub(crate) struct Hud;

#[derive(Component)]
pub(crate) struct MainMenuUi;

#[derive(Component)]
pub(crate) struct PauseDialogUi;

#[derive(Component, Clone, Copy)]
pub(crate) enum MenuAction {
    Start,
    Launch,
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
    players: Query<&Player>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let mut scores: Vec<(usize, u32)> = players.iter().map(|p| (p.id, p.score)).collect();
    scores.sort_by_key(|(id, _)| *id);
    let score_line = scores
        .iter()
        .map(|(id, score)| format!("P{}: {}", id + 1, score))
        .collect::<Vec<_>>()
        .join("   ");
    let remaining = (ROUND_SECONDS - round.timer.elapsed_secs()).max(0.0).ceil() as u32;

    hud.0 = if round.finished {
        let winner = scores
            .iter()
            .max_by_key(|(_, score)| *score)
            .map(|(id, _)| id + 1)
            .unwrap_or(1);
        format!("TIME!  Player {winner} wins\n{score_line}\nPress R to restart")
    } else {
        format!(
            "{remaining:02}s   {score_line}\nSteer at junctions  |  Fire to shoot  |  Hit opponents from behind"
        )
    };
}

pub(crate) fn spawn_main_menu(mut commands: Commands) {
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
            parent.spawn(menu_button("START", MenuAction::Start));
            parent.spawn(menu_button("EXIT", MenuAction::Exit));
            parent.spawn((
                Text::new("Enter: Start   |   Escape: Exit"),
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
                    panel.spawn(menu_button("RESUME", MenuAction::Resume));
                    panel.spawn(menu_button("RETURN TO MENU", MenuAction::ReturnToMenu));
                    panel.spawn((
                        Text::new("Escape: Resume"),
                        TextFont {
                            font_size: FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.58, 0.65, 0.76)),
                    ));
                });
        });
}

pub(super) fn menu_button(label: &'static str, action: MenuAction) -> impl Bundle {
    (
        Button,
        action,
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

pub(crate) fn update_menu_buttons(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::Pressed => Color::srgb(0.90, 0.55, 0.16),
            Interaction::Hovered => Color::srgb(0.22, 0.34, 0.54),
            Interaction::None => Color::srgb(0.13, 0.18, 0.29),
        };
    }
}

pub(crate) fn handle_menu_actions(
    mut commands: Commands,
    buttons: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    pause_dialogs: Query<Entity, With<PauseDialogUi>>,
    mut pause: ResMut<PauseState>,
    roster: Res<Roster>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

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
                for entity in &pause_dialogs {
                    commands.entity(entity).despawn();
                }
            }
            MenuAction::ReturnToMenu => next_state.set(AppState::MainMenu),
        }
    }
}

pub(crate) fn main_menu_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if keys.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Lobby);
    } else if keys.just_pressed(KeyCode::Escape) {
        commands.write_message(AppExit::Success);
    }
}

pub(crate) fn toggle_pause_dialog(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut pause: ResMut<PauseState>,
    dialogs: Query<Entity, With<PauseDialogUi>>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
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
