//! Application configuration and explicit system order.

mod arena;
mod batch;
mod combat;
mod explosions;
mod game;
mod match_settings;
mod players;
mod powerups;
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
        .init_resource::<powerups::Powerups>()
        .insert_resource(RailMap::random())
        .init_resource::<PauseState>()
        .init_resource::<Roster>()
        .init_resource::<match_settings::MatchSettings>()
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
        // After the scene has built the forest; runs again whenever the
        // automatic quality changes the forest's density.
        .add_systems(
            PostUpdate,
            (arena::apply_arena, scenery::apply_forest_density).chain(),
        )
        .add_systems(OnEnter(AppState::MainMenu), ui::spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), ui::despawn_main_menu)
        .add_systems(OnEnter(AppState::Lobby), ui::spawn_lobby)
        .add_systems(OnExit(AppState::Lobby), ui::despawn_lobby)
        .add_systems(OnEnter(AppState::Playing), game::start_new_game)
        .add_systems(
            OnExit(AppState::Playing),
            (ui::close_pause_dialog, ui::despawn_match_over_ui),
        )
        // Every menu reads one frame of device-agnostic intent, so it is
        // gathered once, ahead of the screens that act on it.
        .add_systems(Update, ui::gather_menu_input)
        .add_systems(
            Update,
            (
                ui::navigate_menu,
                // Ahead of the actions, so that Start opening the dialog and
                // Start confirming a button in it stay one frame apart.
                ui::toggle_pause_dialog.run_if(in_state(AppState::Playing)),
                ui::handle_menu_actions,
                ui::update_menu_buttons,
                ui::main_menu_shortcuts.run_if(in_state(AppState::MainMenu)),
            )
                .chain()
                .after(ui::gather_menu_input),
        )
        .add_systems(
            Update,
            (
                ui::lobby_input,
                ui::lobby_shortcuts,
                ui::refresh_lobby,
                ui::refresh_setting_labels,
            )
                .chain()
                .after(ui::gather_menu_input)
                .run_if(in_state(AppState::Lobby)),
        )
        .add_systems(
            Update,
            (
                players::human_input,
                players::bot_input,
                powerups::tick_and_drop.run_if(game::round_is_live),
                players::move_players,
                players::handle_player_collisions,
                powerups::collect.run_if(game::round_is_live),
                combat::fire_projectiles.run_if(game::round_is_live),
                combat::move_projectiles.run_if(game::round_is_live),
                combat::strike_scenery.run_if(game::round_is_live),
                combat::detect_hits.run_if(game::round_is_live),
                explosions::run_detonations,
                explosions::animate_fireballs,
                explosions::animate_smoke,
                explosions::animate_flashes,
                explosions::move_debris,
                explosions::fade_scorches,
                game::tick_round,
                game::restart_round,
                ui::update_hud,
                ui::sync_match_over_ui,
            )
                .chain()
                .run_if(in_state(AppState::Playing))
                .after(ui::handle_menu_actions)
                .run_if(ui::gameplay_is_running),
        )
        .run();
}
