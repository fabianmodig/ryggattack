//! Application configuration and explicit system order.

mod combat;
mod game;
mod players;
mod scene;
mod tracks;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use game::{AppState, Round};
use players::Roster;
use tracks::RailMap;
use ui::{LobbyLatch, PauseState};

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.045, 0.075)))
        .insert_resource(Round::default())
        .insert_resource(RailMap::random())
        .init_resource::<PauseState>()
        .init_resource::<Roster>()
        .init_resource::<LobbyLatch>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Ryggattack — POC".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .init_state::<AppState>()
        .add_systems(Startup, scene::setup)
        .add_systems(OnEnter(AppState::MainMenu), ui::spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), ui::despawn_main_menu)
        .add_systems(OnEnter(AppState::Lobby), ui::spawn_lobby)
        .add_systems(OnExit(AppState::Lobby), ui::despawn_lobby)
        .add_systems(OnEnter(AppState::Playing), game::start_new_game)
        .add_systems(OnExit(AppState::Playing), ui::close_pause_dialog)
        .add_systems(
            Update,
            (
                ui::update_menu_buttons,
                ui::handle_menu_actions,
                ui::main_menu_keyboard.run_if(in_state(AppState::MainMenu)),
                ui::toggle_pause_dialog.run_if(in_state(AppState::Playing)),
            ),
        )
        .add_systems(
            Update,
            (ui::lobby_input, ui::lobby_keyboard, ui::refresh_lobby)
                .chain()
                .run_if(in_state(AppState::Lobby)),
        )
        .add_systems(
            Update,
            (
                game::tick_round,
                players::human_input,
                players::bot_input,
                players::move_players,
                players::handle_player_collisions,
                combat::fire_projectiles,
                combat::move_projectiles,
                combat::detect_hits,
                game::restart_round,
                ui::update_hud,
            )
                .chain()
                .run_if(in_state(AppState::Playing))
                .run_if(ui::gameplay_is_running),
        )
        .run();
}
