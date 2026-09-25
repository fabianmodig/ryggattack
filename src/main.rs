//! Application configuration and explicit system order.

mod batch;
mod combat;
mod explosions;
mod game;
mod players;
mod scene;
mod scenery;
mod settings;
mod tracks;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use game::{AppState, Round};
use players::Roster;
use tracks::RailMap;
use ui::{LobbyLatch, MenuFocus, MenuInput, PauseState, StickLatch};

fn main() {
    App::new()
        // The fog colour, so that anything past the tree line fades into it.
        .insert_resource(ClearColor(Color::srgb(0.62, 0.72, 0.66)))
        .insert_resource(Round::default())
        .insert_resource(RailMap::random())
        .init_resource::<PauseState>()
        .init_resource::<Roster>()
        .init_resource::<LobbyLatch>()
        .init_resource::<MenuInput>()
        .init_resource::<MenuFocus>()
        .init_resource::<StickLatch>()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Ryggattack — POC".into(),
                        resolution: WindowResolution::new(1280, 720),
                        present_mode: PresentMode::AutoVsync,
                        // The WebAssembly build renders into the canvas that
                        // `web/index.html` provides; ignored on desktop.
                        canvas: Some("#ryggattack-canvas".into()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(settings::SettingsPlugin)
        .init_state::<AppState>()
        .add_systems(Startup, scene::setup)
        .add_systems(OnEnter(AppState::MainMenu), ui::spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), ui::despawn_main_menu)
        .add_systems(OnEnter(AppState::Lobby), ui::spawn_lobby)
        .add_systems(OnExit(AppState::Lobby), ui::despawn_lobby)
        .add_systems(OnEnter(AppState::Playing), game::start_new_game)
        .add_systems(OnExit(AppState::Playing), ui::close_pause_dialog)
        // Every menu reads one frame of device-agnostic intent, so it is
        // gathered once, ahead of the screens that act on it.
        .add_systems(Update, ui::gather_menu_input)
        .add_systems(
            Update,
            (
                ui::navigate_menu,
                // Ahead of the actions, so that Start opening the dialog and
                // Start confirming a button in it stay one frame apart.
                ui::toggle_pause_dialog
                    .run_if(in_state(AppState::Playing))
                    .run_if(not(ui::settings_are_open)),
                ui::handle_menu_actions,
                ui::adjust_focused_setting,
                ui::update_menu_buttons,
                ui::main_menu_shortcuts
                    .run_if(in_state(AppState::MainMenu))
                    .run_if(not(ui::settings_are_open)),
                // Last, so that the Escape that closes the settings is not
                // also read as closing the screen they return to.
                ui::settings_shortcuts,
                ui::refresh_setting_values,
            )
                .chain()
                .after(ui::gather_menu_input),
        )
        .add_systems(
            Update,
            (ui::lobby_input, ui::lobby_shortcuts, ui::refresh_lobby)
                .chain()
                .after(ui::gather_menu_input)
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
                combat::strike_scenery,
                combat::detect_hits,
                explosions::run_detonations,
                explosions::animate_fireballs,
                explosions::animate_smoke,
                explosions::animate_flashes,
                explosions::move_debris,
                explosions::fade_scorches,
                game::restart_round,
                ui::update_hud,
            )
                .chain()
                .run_if(in_state(AppState::Playing))
                .run_if(ui::gameplay_is_running),
        )
        .run();
}
