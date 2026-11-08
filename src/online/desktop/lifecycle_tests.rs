use super::*;
use crate::{
    arena, game, input, match_settings, players, powerups, scene, scenery, settings, simulation,
    tracks, ui,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<settings::VideoSettings>()
        .init_resource::<match_settings::MatchSettings>()
        .init_resource::<game::Round>()
        .init_resource::<powerups::Powerups>()
        .init_resource::<players::Roster>()
        .init_resource::<input::SeatInputs>()
        .init_resource::<input::TouchInput>()
        .init_resource::<ui::PauseState>()
        .init_resource::<ui::MenuInput>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(tracks::RailMap::from_seed(42))
        .add_plugins((DesktopOnlinePlugin, simulation::SimulationPlugin))
        .add_systems(Startup, scene::setup)
        .add_systems(
            PostUpdate,
            (arena::apply_arena, scenery::apply_forest_density)
                .chain()
                .run_if(not(in_state(AppState::Online))),
        );
    app.update();
    app
}
fn click(app: &mut App, action: Action) {
    let entity = app
        .world_mut()
        .query_filtered::<(Entity, &Action), With<Button>>()
        .iter(app.world())
        .find(|(_, a)| **a == action)
        .unwrap_or_else(|| panic!("no {action:?} button"))
        .0;
    *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
    app.update();
    if let Some(mut interaction) = app.world_mut().get_mut::<Interaction>(entity) {
        *interaction = Interaction::None;
    }
}
fn until(app: &mut App, predicate: impl Fn(&World) -> bool) {
    let end = Instant::now() + Duration::from_secs(8);
    while !predicate(app.world()) {
        assert!(
            Instant::now() < end,
            "native lifecycle timed out: {}",
            app.world()
                .get_non_send::<NetworkClient>()
                .map_or("no client", |c| c.status.as_str())
        );
        app.update();
        std::thread::sleep(Duration::from_millis(2));
    }
}
struct Backend {
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    address: String,
}
impl Backend {
    fn start() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let worker = std::thread::spawn(move || {
            let mut server = crate::server::Server::bind("127.0.0.1:0").unwrap();
            tx.send(server.local_addr()).unwrap();
            while !flag.load(Ordering::Relaxed) {
                server.tick();
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        Self {
            stop,
            worker: Some(worker),
            address: format!("ws://{}", rx.recv().unwrap()),
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn native_join_uses_room_code_and_hides_host_only_actions() {
    let backend = Backend::start();
    let mut host = NetworkClient::connect(&backend.address, "Host").unwrap();
    let end = Instant::now() + Duration::from_secs(8);
    while host.lobby.is_none() {
        assert!(Instant::now() < end);
        host.pump(default());
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut app = app();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    {
        let mut entry = app.world_mut().resource_mut::<Entry>();
        entry.address = backend.address.clone();
        entry.code = host.lobby.as_ref().unwrap().code.clone();
    }
    click(&mut app, Action::Join);
    until(&mut app, |w| w.non_send::<NetworkClient>().lobby.is_some());
    assert_eq!(app.world().non_send::<NetworkClient>().room, host.room);
    let actions: Vec<_> = app
        .world_mut()
        .query_filtered::<&Action, With<Button>>()
        .iter(app.world())
        .copied()
        .collect();
    assert!(actions.contains(&Action::Ready));
    let labels: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    for label in ["COPY INVITE LINK", "COPY ROOM CODE"] {
        assert!(labels.iter().any(|text| text == label), "missing {label}");
    }
    // Minimal/headless apps intentionally have no OS clipboard resource.
    click(&mut app, Action::CopyCode);
    assert!(
        app.world()
            .resource::<Desktop>()
            .notice
            .to_lowercase()
            .contains("clipboard")
    );
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0.to_lowercase().contains("clipboard")),
        "clipboard failure must be visible in the lobby"
    );
    for action in [Action::Bots, Action::Target, Action::Arena] {
        assert!(!actions.contains(&action));
    }
    click(&mut app, Action::Leave);
    until(&mut app, |w| w.get_non_send::<NetworkClient>().is_none());
    host.leave();
}

#[test]
fn resuming_input_does_not_fire_with_the_menu_accept_key_still_held() {
    let backend = Backend::start();
    let mut app = app();
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    app.world_mut().resource_mut::<Entry>().address = backend.address.clone();
    click(&mut app, Action::Create);
    until(&mut app, |w| w.non_send::<NetworkClient>().lobby.is_some());
    click(&mut app, Action::Ready);
    until(&mut app, |w| w.non_send::<NetworkClient>().frame.is_some());
    app.world_mut().resource_mut::<ui::MenuInput>().cancel = true;
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Space);
    let seat = app.world().non_send::<NetworkClient>().seat.unwrap();
    let ack = app
        .world()
        .non_send::<NetworkClient>()
        .frame
        .as_ref()
        .unwrap()
        .ack[seat];
    click(&mut app, Action::Resume);
    until(&mut app, |w| {
        w.non_send::<NetworkClient>().frame.as_ref().unwrap().ack[seat] > ack + 1
    });
    let client = app.world().non_send::<NetworkClient>();
    assert_eq!(
        client.frame.as_ref().unwrap().carts[client.seat.unwrap()].shot_cooldown,
        0.0,
        "menu confirmation must remain neutral until gameplay controls are released"
    );
}

#[test]
fn native_menu_runs_shared_rooms_and_restores_local_scene_repeatedly() {
    let backend = Backend::start();
    let mut app = app();
    let camera = app
        .world_mut()
        .query_filtered::<Entity, With<settings::WorldCamera>>()
        .single(app.world())
        .unwrap();
    let seed = app.world().resource::<tracks::RailMap>().seed;
    for _ in 0..2 {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Online);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&players::Player>()
                .iter(app.world())
                .count(),
            0,
            "offline carts cannot coexist with replicas"
        );
        app.world_mut().resource_mut::<Entry>().address = backend.address.clone();
        click(&mut app, Action::Create);
        until(&mut app, |w| {
            w.get_non_send::<NetworkClient>()
                .is_some_and(|c| c.lobby.is_some())
        });
        let code = app
            .world()
            .non_send::<NetworkClient>()
            .lobby
            .as_ref()
            .unwrap()
            .code
            .clone();
        let mut peer =
            NetworkClient::connect_room(&backend.address, "Browser protocol peer", Some(&code))
                .unwrap();
        let end = Instant::now() + Duration::from_secs(8);
        while peer.lobby.as_ref().is_none_or(|l| l.humans.len() != 2) {
            assert!(Instant::now() < end);
            peer.pump(default());
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
        until(&mut app, |w| {
            w.non_send::<NetworkClient>()
                .lobby
                .as_ref()
                .is_some_and(|l| l.humans.len() == 2)
        });
        assert_eq!(peer.room, app.world().non_send::<NetworkClient>().room);
        click(&mut app, Action::Arena);
        until(&mut app, |w| {
            w.non_send::<NetworkClient>()
                .lobby
                .as_ref()
                .unwrap()
                .settings
                .arena
                == match_settings::Arena::Volcano
        });
        click(&mut app, Action::Ready);
        peer.command(crate::network::ClientMessage::Ready(true))
            .unwrap();
        until(&mut app, |w| w.non_send::<NetworkClient>().frame.is_some());
        assert!(app.world().resource::<super::super::View>().built);
        assert_eq!(
            app.world_mut()
                .query::<&players::Player>()
                .iter(app.world())
                .count(),
            4
        );
        // Online state must not enable the local authoritative schedule.
        let round_before = app.world().resource::<game::Round>().wins;
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().resource::<game::Round>().wins, round_before);
        app.world_mut().resource_mut::<ui::MenuInput>().cancel = true;
        app.update();
        assert!(app.world().resource::<Desktop>().menu);
        click(&mut app, Action::Leave);
        until(&mut app, |w| w.get_non_send::<NetworkClient>().is_none());
        // The surviving peer must observe removal, not a disconnected reservation.
        let end = Instant::now() + Duration::from_secs(8);
        while peer.lobby.as_ref().is_none_or(|l| l.humans.len() != 1) {
            assert!(Instant::now() < end);
            peer.pump(default());
            app.update();
            std::thread::sleep(Duration::from_millis(2));
        }
        peer.leave();
        click(&mut app, Action::Back);
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::MainMenu
        );
        assert!(
            app.world().get_entity(camera).is_ok(),
            "persistent camera and render settings survive mode changes"
        );
        assert_eq!(app.world().resource::<tracks::RailMap>().seed, seed);
        assert_eq!(
            app.world_mut()
                .query::<&players::Player>()
                .iter(app.world())
                .count(),
            4
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ui::Hud>>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<super::super::environment::Environment>>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<scenery::Backdrop>>()
                .iter(app.world())
                .count()
                > 0
        );
    }
}
