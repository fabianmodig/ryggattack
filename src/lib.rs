//! Application configuration and explicit system order.

mod arena;
mod batch;
mod combat;
mod explosions;
mod game;
pub mod headless;
pub mod input;
#[cfg(not(target_arch = "wasm32"))]
pub mod management;
mod match_settings;
pub mod net_client;
pub mod network;
mod online;
mod players;
mod powerups;
pub mod prediction;
pub mod rooms;
mod scene;
mod scenery;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
mod settings;
mod simulation;
mod tracks;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

use game::{AppState, Round};
use players::Roster;
use tracks::RailMap;
use ui::{LobbyLatch, MenuFocus, MenuInput, PauseState, StickLatch};

pub fn run_client() {
    if let Some((address, name)) = online::entry() {
        online::run(address, name);
        return;
    }
    App::new()
        // The fog colour, so that anything past the tree line fades into it.
        .insert_resource(ClearColor(Color::srgb(0.62, 0.72, 0.66)))
        .insert_resource(Round::default())
        .init_resource::<powerups::Powerups>()
        .insert_resource(RailMap::random())
        .init_resource::<PauseState>()
        .init_resource::<Roster>()
        .init_resource::<input::SeatInputs>()
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
        .add_systems(PreUpdate, browser_local_menu)
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
        .add_systems(
            PreUpdate,
            ui::gather_menu_input
                .after(bevy::input::InputSystems)
                .after(bevy::ui::UiSystems::Focus),
        )
        .add_systems(
            PreUpdate,
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
            PreUpdate,
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
        .add_plugins((simulation::SimulationPlugin, simulation::PresentationPlugin))
        .add_systems(
            PreUpdate,
            players::human_input
                .after(ui::handle_menu_actions)
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(
            Update,
            (game::restart_round, ui::update_hud, ui::sync_match_over_ui)
                .chain()
                .run_if(in_state(AppState::Playing))
                .run_if(ui::gameplay_is_running),
        )
        .run();
}

// Browser selection changes only the local application's existing state.
// Online launch selects a separate presentation-only adapter at startup.
fn browser_local_menu(
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    windows: Query<&Window>,
    mut scale: ResMut<UiScale>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let global = js_sys::global();
        scale.0 = if *state.get() == AppState::Lobby {
            windows.iter().next().map_or(1.0, |w| {
                (w.width() / 960.0).min(w.height() / 650.0).min(1.0)
            })
        } else {
            1.0
        };
        js_sys::Reflect::set(
            &global,
            &"ryggattackLocalScreen".into(),
            &wasm_bindgen::JsValue::from_str(&format!("{:?}", state.get())),
        )
        .ok();
        if js_sys::Reflect::get(&global, &"ryggattackLocalAction".into())
            .ok()
            .and_then(|v| v.as_string())
            .as_deref()
            == Some("local")
        {
            js_sys::Reflect::set(
                &global,
                &"ryggattackLocalAction".into(),
                &wasm_bindgen::JsValue::NULL,
            )
            .ok();
            if *state.get() == AppState::MainMenu {
                next.set(AppState::Lobby);
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (state, &mut next, windows, &mut scale);
    }
}
