//! A presentation-only online app: intentionally does not install any gameplay,
//! local pause, scoring, RNG, collision, or round-reset system.
use crate::{input::ControlIntent, net_client::NetworkClient, network::Frame};
use bevy::prelude::*;
mod environment;
#[derive(Resource, Default)]
struct View {
    edges: Option<Vec<(usize, usize)>>,
    frame: Option<Frame>,
    previous: Option<Frame>,
    alpha: f32,
    built: bool,
    epoch: u64,
    reset_replicas: bool,
    reset_scene: bool,
    arena: crate::match_settings::Arena,
    environment: Option<crate::match_settings::Arena>,
    identity: (Option<u64>, Option<u64>, Option<usize>),
}
#[derive(Component)]
struct Replica(u64);
#[derive(Component)]
struct Ground;
type RoomVisualEntity = (
    Entity,
    Option<&'static Replica>,
    Option<&'static environment::Environment>,
);
type RoomVisualRoots = (
    Or<(
        With<Mesh3d>,
        With<crate::players::Player>,
        With<Replica>,
        With<environment::Environment>,
    )>,
    Without<ChildOf>,
    Without<Ground>,
);
#[derive(Component)]
struct Status;
struct VisualAssets {
    arena: crate::match_settings::Arena,
    cube: Handle<Mesh>,
    missiles: crate::combat::MissileAssets,
    forest: crate::scenery::ForestAssets,
    fireball: Handle<Mesh>,
    ring: Handle<Mesh>,
    materials: [Handle<StandardMaterial>; 5],
}

#[cfg(any(not(target_arch = "wasm32"), test))]
fn native_entry(args: &[String]) -> Option<(String, String)> {
    let address = args.windows(2).find(|a| a[0] == "--online")?[1].clone();
    let name = args
        .windows(2)
        .find(|a| a[0] == "--name")
        .map_or("Player".into(), |a| a[1].clone());
    Some((address, name))
}
pub(crate) fn entry() -> Option<(String, String)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native_entry(&std::env::args().skip(1).collect::<Vec<_>>())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window()?;
        let params =
            web_sys::UrlSearchParams::new_with_str(&window.location().search().ok()?).ok()?;
        Some((
            params.get("online")?,
            params.get("name").unwrap_or_else(|| "Player".into()),
        ))
    }
}
#[derive(Resource)]
struct Entry {
    address: String,
    name: String,
    code: String,
}
fn initial_room() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|a| a[0] == "--room")
            .map(|a| a[1].clone())
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
            .and_then(|p| p.get("room"))
            .filter(|s| !s.is_empty())
    }
}
fn lobby_key(
    key: KeyCode,
    lobby: &crate::rooms::Lobby,
    participant: u64,
) -> Option<crate::network::ClientMessage> {
    use crate::{network::ClientMessage, rooms::Phase};
    if lobby.phase == Phase::Running {
        return None;
    }
    if key == KeyCode::KeyR {
        return lobby
            .humans
            .iter()
            .find(|h| h.id == participant)
            .map(|h| ClientMessage::Ready(!h.ready));
    }
    if lobby.host != participant {
        return None;
    }
    let mut settings = lobby.settings.clone();
    match key {
        KeyCode::KeyB => settings.bots = (settings.bots + 1) % (5 - lobby.humans.len()),
        KeyCode::KeyT => {
            settings.wins_target = match settings.wins_target {
                1 => 3,
                3 => 5,
                5 => 7,
                _ => 1,
            }
        }
        KeyCode::KeyV => {
            settings.arena = match settings.arena {
                crate::headless::Arena::Forest => crate::headless::Arena::Volcano,
                _ => crate::headless::Arena::Forest,
            }
        }
        _ => return None,
    }
    Some(ClientMessage::Configure(settings))
}
fn online_controls(
    mut commands: Commands,
    mut client: NonSendMut<NetworkClient>,
    mut entry: ResMut<Entry>,
    keys: Res<ButtonInput<KeyCode>>,
    mut view: ResMut<View>,
    scene_roots: Query<RoomVisualEntity, RoomVisualRoots>,
) {
    let mut action = None;
    #[cfg(target_arch = "wasm32")]
    {
        let global = js_sys::global();
        if let Ok(value) = js_sys::Reflect::get(&global, &"ryggattackCommand".into()) {
            if let Some(text) = value.as_string() {
                action = crate::network::decode_client(&text).ok();
            }
            js_sys::Reflect::set(
                &global,
                &"ryggattackCommand".into(),
                &wasm_bindgen::JsValue::NULL,
            )
            .ok();
        }
    }
    if canvas_focused() {
        if keys.just_pressed(KeyCode::KeyL) && client.seat.is_some() {
            client.leave();
        }
        if let (Some(lobby), Some(participant)) = (&client.lobby, client.participant) {
            for key in keys.get_just_pressed() {
                if let Some(cmd) = lobby_key(*key, lobby, participant) {
                    action = Some(cmd);
                }
            }
        }
        if client.seat.is_none() {
            for (key, ch) in [
                (KeyCode::Digit0, '0'),
                (KeyCode::Digit1, '1'),
                (KeyCode::Digit2, '2'),
                (KeyCode::Digit3, '3'),
                (KeyCode::Digit4, '4'),
                (KeyCode::Digit5, '5'),
                (KeyCode::Digit6, '6'),
                (KeyCode::Digit7, '7'),
                (KeyCode::Digit8, '8'),
                (KeyCode::Digit9, '9'),
                (KeyCode::KeyA, 'a'),
                (KeyCode::KeyB, 'b'),
                (KeyCode::KeyC, 'c'),
                (KeyCode::KeyD, 'd'),
                (KeyCode::KeyE, 'e'),
                (KeyCode::KeyF, 'f'),
            ] {
                if keys.just_pressed(key) && entry.code.len() < 20 {
                    entry.code.push(ch);
                }
            }
            if keys.just_pressed(KeyCode::Backspace) {
                entry.code.pop();
            }
            if keys.just_pressed(KeyCode::F2) || keys.just_pressed(KeyCode::Enter) {
                let room = if keys.just_pressed(KeyCode::F2) {
                    None
                } else {
                    Some(entry.code.as_str())
                };
                match NetworkClient::connect_room(&entry.address, &entry.name, room) {
                    Ok(next) => {
                        client.leave();
                        *client = next;
                        // The railway and cart models are reusable when the next
                        // Welcome has the same topology. Hide carts via the empty
                        // frame; never reallocate their materials on every rejoin.
                        let built = view.built;
                        *view = View {
                            built,
                            edges: view.edges.clone(),
                            reset_replicas: true,
                            ..default()
                        };
                        for (entity, replica, environment) in &scene_roots {
                            if !built || replica.is_some() || environment.is_some() {
                                commands.entity(entity).despawn();
                            }
                        }
                    }
                    Err(e) => client.notice = e,
                }
            }
        }
    }
    if let Some(action) = action {
        if matches!(action, crate::network::ClientMessage::Leave) {
            client.leave();
        } else if let Err(error) = client.command(action) {
            client.notice = error;
        }
    }
}
fn initial_client(address: &str, name: &str, room: Option<&str>) -> NetworkClient {
    NetworkClient::connect_room(address, name, room).unwrap_or_else(|error| {
        let mut client = NetworkClient::connect(address, "Player").expect("valid fallback name");
        client.leave();
        client.status = error;
        client
    })
}
pub(crate) fn run(address: String, name: String) {
    let room = initial_room();
    let client = initial_client(&address, &name, room.as_deref());
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.10, 0.12)))
        .insert_non_send(client)
        .insert_resource(Entry {
            address,
            name,
            code: String::new(),
        })
        .init_resource::<View>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ryggattack — Private online room".into(),
                resolution: (1100, 720).into(),
                canvas: Some("#ryggattack-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                online_controls,
                network_input,
                render_snapshot,
                environment::present,
            )
                .chain(),
        )
        .run();
}
fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 16.5, 15.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.,
            ..default()
        },
        Transform::from_xyz(4., 12., 6.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn(AmbientLight {
        brightness: 400.,
        ..default()
    });

    commands.spawn((
        Text::new("Connecting…"),
        TextFont {
            font_size: 20.0.into(),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(12.),
            left: px(12.),
            ..default()
        },
        Status,
    ));
}
fn presentation_ready(frames: &mut u8) -> bool {
    let ready = *frames >= 2;
    *frames = frames.saturating_add(1);
    ready
}
fn gameplay_active(menu: bool, window_focused: bool, canvas_focused: bool) -> bool {
    !menu && window_focused && canvas_focused
}
// Explicit presentation whitelist: never serialize NetworkClient, Lobby, or Frame
// wholesale here. In particular, claims, invitation codes and scenery stay out.
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn hud_snapshot(client: &NetworkClient, menu: bool) -> serde_json::Value {
    let lobby = client.lobby.as_ref().filter(|lobby| {
        Some(lobby.id) == client.room
            && lobby.humans.iter().any(|human| {
                Some(human.id) == client.participant && Some(human.seat) == client.seat
            })
    });
    let frame = lobby
        .filter(|lobby| lobby.phase == crate::rooms::Phase::Running)
        .and(client.frame.as_ref())
        .map(|frame| {
            serde_json::json!({
                "tick": frame.tick,
                "simulation_tick": frame.simulation_tick,
                "names": frame.names,
                "wins": frame.wins,
                "winner": frame.winner,
                "round_finished": frame.round_finished,
                "transition": frame.transition,
                "carts": frame.carts.iter().map(|cart| serde_json::json!({
                    "id": cart.id, "active": cart.active, "alive": cart.alive,
                    "speed_seconds": cart.speed_seconds, "all_hits_seconds": cart.all_hits_seconds
                })).collect::<Vec<_>>()
            })
        });
    serde_json::json!({
        "lobby": lobby.map(|lobby| serde_json::json!({
            "id": lobby.id, "phase": lobby.phase
        })),
        "frame": frame, "seat": lobby.and(client.seat),
        "status": client.status, "notice": client.notice,
        "menu": menu, "rtt_ms": client.rtt_ms
    })
}
#[cfg(any(target_arch = "wasm32", test))]
fn publish_hud(client: &NetworkClient, menu: bool, write: impl FnOnce(&str, &str) -> bool) -> bool {
    write("ryggattackHud", &hud_snapshot(client, menu).to_string())
}
fn canvas_status_text(label: String, dom_ready: bool) -> String {
    if dom_ready { String::new() } else { label }
}
fn canvas_focused() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .is_some_and(|e| e.id() == "ryggattack-canvas")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}
#[allow(clippy::too_many_arguments)]
fn network_input(
    mut client: NonSendMut<NetworkClient>,
    mut view: ResMut<View>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    windows: Query<&Window>,
    mut status: Query<&mut Text, With<Status>>,
    mut menu: Local<bool>,
    mut fire: Local<bool>,
    mut warmup: Local<u8>,
    entry: Res<Entry>,
) {
    // In WASM the first GPU submissions can block the JS thread for seconds.
    // Yield initial presentations before opening a transport whose callbacks
    // need that same event loop; do not spend its deadline compiling shaders.
    if !presentation_ready(&mut warmup) {
        return;
    }
    let canvas_focused = canvas_focused();
    if canvas_focused && keys.just_pressed(KeyCode::Escape) {
        *menu = !*menu;
    }
    let active = client
        .lobby
        .as_ref()
        .is_some_and(|l| l.phase == crate::rooms::Phase::Running)
        && gameplay_active(*menu, windows.iter().all(|w| w.focused), canvas_focused);
    let mut intent = ControlIntent::default();
    if active {
        intent.horizontal = (keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight)) as u8
            as f32
            - (keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft)) as u8 as f32;
        intent.vertical = (keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp)) as u8
            as f32
            - (keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown)) as u8 as f32;
        *fire |= keys.just_pressed(KeyCode::Space);
        intent.fire = keys.pressed(KeyCode::Space) || *fire;
        if let Some(pad) = pads.iter().next() {
            if intent.horizontal == 0. {
                intent.horizontal = pad.left_stick().x;
            }
            if intent.vertical == 0. {
                intent.vertical = pad.left_stick().y;
            }
            intent.fire |= pad.pressed(GamepadButton::South);
        }
    } else {
        *fire = false;
    }
    if client.pump(intent) {
        *fire = false;
    }
    sync_visual_boundary(&client, &mut view);
    // One snapshot interval of visual interpolation, no extrapolation/prediction.
    view.alpha = (client.received.elapsed().as_secs_f32() / 0.0625).clamp(0., 1.);
    let detail=client.frame.as_ref().map(|f|format!("tick {}  wins {:?}  winner {:?}\nPlayers: {:?}\nPowerups: speed {:.1}s / all-hits {:.1}s; drop {:.1}s",f.tick,f.wins,f.winner,f.names,client.seat.map_or(0.,|s|f.carts[s].speed_seconds),client.seat.map_or(0.,|s|f.carts[s].all_hits_seconds),f.until_drop)).unwrap_or_default();
    let lobby_detail=client.lobby.as_ref().map(|l|format!("Room {}  ID {}  {:?}\nShare: ?online={}&room={}\nHost: {}  Arena {:?}  First to {}  Bots {}\n{}\nR: ready/unready • Host B: bots / T: target / V: arena • L: leave",l.code,l.id,l.phase,entry.address,l.code,l.humans.iter().find(|h|h.id==l.host).map_or("?",|h|h.name.as_str()),l.settings.arena,l.settings.wins_target,l.settings.bots,l.humans.iter().map(|h|format!("{}: {}{} [{}]",h.seat+1,h.name,if h.id==l.host{" (host)"}else{""},if !h.connected {"BOT / reconnect reserved"}else if h.ready{"ready"}else{"not ready"})).collect::<Vec<_>>().join("\n"))).unwrap_or_else(||format!("F2: create room • Type code + Enter: join\nRoom code: {} (Backspace to edit)\nName: {} (set --name on launch) • Backend {}",entry.code,entry.name,entry.address));
    let label = format!(
        "{}  ack RTT {:.0}ms\n{}\nWASD/arrows: junction • Space: fire • Esc: {} (server keeps running)",
        client.status,
        client.rtt_ms.unwrap_or(0.),
        format_args!("{}\n{}\n{}", lobby_detail, client.notice, detail),
        if *menu {
            "resume input"
        } else {
            "suppress input"
        }
    );
    #[cfg(not(target_arch = "wasm32"))]
    let dom_hud_ready = false;
    #[cfg(target_arch = "wasm32")]
    let dom_hud_ready = {
        let global = js_sys::global();
        publish_hud(&client, *menu, |key, value| {
            js_sys::Reflect::set(
                &global,
                &key.into(),
                &wasm_bindgen::JsValue::from_str(value),
            )
            .unwrap_or(false)
        }) && js_sys::Reflect::get(&global, &"ryggattackHudMounted".into())
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    };
    let label = canvas_status_text(label, dom_hud_ready);
    for mut text in &mut status {
        **text = label.clone();
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Reflect::set(&js_sys::global(), &"ryggattackRoom".into(), &wasm_bindgen::JsValue::from_str(&serde_json::json!({"lobby":client.lobby,"participant":client.participant,"status":client.status,"notice":client.notice}).to_string())).ok();
        js_sys::Reflect::set(
            &js_sys::global(),
            &"ryggattackOnline".into(),
            &wasm_bindgen::JsValue::from_str(&format!(
                "{}|{}",
                client.seat.map_or(99, |s| s),
                client.frame.as_ref().map_or(0, |f| f.tick)
            )),
        )
        .ok();
    }
}
fn sync_visual_boundary(client: &NetworkClient, view: &mut View) {
    if let Some(edges) = &client.edges
        && view.edges.as_ref() != Some(edges)
    {
        view.reset_scene = view.built;
        view.built = false;
        view.reset_replicas = true;
        view.edges = Some(edges.clone());
    }
    let identity = (client.room, client.participant, client.seat);
    if view.epoch != client.visual_epoch || view.identity != identity {
        view.reset_replicas = true;
    }
    view.epoch = client.visual_epoch;
    view.identity = identity;
    let lobby = client.lobby.as_ref().filter(|lobby| {
        Some(lobby.id) == client.room
            && lobby.humans.iter().any(|human| {
                Some(human.id) == client.participant && Some(human.seat) == client.seat
            })
    });
    if let Some(lobby) = lobby {
        view.arena = lobby.settings.arena;
    }
    view.environment = lobby.map(|lobby| lobby.settings.arena);
    let running = lobby.is_some_and(|l| l.phase == crate::rooms::Phase::Running);
    view.frame = running.then(|| client.frame.clone()).flatten();
    view.previous = running.then(|| client.previous.clone()).flatten();
}

#[allow(clippy::too_many_arguments)]
fn render_snapshot(
    mut commands: Commands,
    mut view: ResMut<View>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut carts: Query<(&mut crate::players::Player, &mut Transform, &mut Visibility)>,
    mut replicas: Query<(Entity, &Replica, &mut Transform), Without<crate::players::Player>>,
    mut assets: Local<Option<VisualAssets>>,
    scene_roots: Query<Entity, (RoomVisualRoots, Without<Replica>)>,
) {
    if std::mem::take(&mut view.reset_scene) {
        for entity in &scene_roots {
            commands.entity(entity).despawn();
        }
    }
    if !view.built {
        let Some(edges) = view.edges.clone() else {
            return;
        };
        let map = crate::tracks::RailMap::from_edges(0, edges);
        crate::tracks::spawn_tracks(&mut commands, &mut meshes, &mut materials, &map);
        crate::players::spawn_players(&mut commands, &mut meshes, &mut materials, &map);
        view.built = true;
    }
    let assets = assets.get_or_insert_with(|| VisualAssets {
        arena: crate::match_settings::Arena::Forest,
        cube: meshes.add(Cuboid::new(1., 1., 1.)),
        missiles: crate::combat::MissileAssets::new(&mut meshes, &mut materials),
        forest: crate::scenery::ForestAssets::new(&mut meshes, &mut materials),
        fireball: meshes.add(crate::explosions::fireball_mesh()),
        ring: meshes.add(Torus::new(0.28, 0.5)),
        materials: [
            Color::WHITE,
            Color::srgb(1., 0.8, 0.),
            Color::srgb(1., 0.1, 0.7),
            Color::srgb(0.16, 0.5, 0.22),
            Color::srgb(1., 0.25, 0.02),
        ]
        .map(|base_color| {
            materials.add(StandardMaterial {
                base_color,
                unlit: true,
                ..default()
            })
        }),
    });
    if assets.arena != view.arena {
        assets.forest.set_arena(view.arena, &mut materials);
        assets.arena = view.arena;
    }
    let reset = std::mem::take(&mut view.reset_replicas);
    let Some(frame) = &view.frame else {
        // A fresh simulation may restart match-local body IDs. Clear the old
        // presentation at the lobby boundary instead of aliasing next-match
        // projectiles/pickups to meshes left by the previous match.
        for (entity, _, _) in &mut replicas {
            commands.entity(entity).despawn();
        }
        for (_, _, mut visibility) in &mut carts {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    for (mut player, mut transform, mut visibility) in &mut carts {
        let Some(cart) = frame.carts.iter().find(|c| c.id == player.id) else {
            continue;
        };
        let mut position = Vec3::from_array(cart.position);
        let mut forward = Vec3::from_array(cart.forward);
        if let Some(old) = view
            .previous
            .as_ref()
            .and_then(|p| p.carts.iter().find(|c| c.id == cart.id))
        {
            let previous = Vec3::from_array(old.position);
            if old.alive == cart.alive && previous.distance(position) < 2. {
                position = previous.lerp(position, view.alpha);
                forward = Vec3::from_array(old.forward)
                    .lerp(forward, view.alpha)
                    .normalize_or_zero();
            }
        }
        transform.translation = position;
        if forward.length_squared() > 0.5 {
            transform.look_to(forward, Vec3::Y);
        }
        player.alive = cart.alive;
        player.active = cart.active;
        *visibility = if cart.alive && cart.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let mut existing = std::collections::HashSet::new();
    for (entity, id, mut transform) in &mut replicas {
        if reset {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(body) = frame.bodies.iter().find(|b| b.id == id.0) {
            existing.insert(id.0);
            *transform = body_transform(body);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for body in &frame.bodies {
        if existing.contains(&body.id) {
            continue;
        }
        let entity = commands
            .spawn((
                Replica(body.id),
                body_transform(body),
                Visibility::default(),
            ))
            .id();
        if body.kind == 0 {
            crate::combat::missile_visual(
                &mut commands,
                entity,
                &assets.missiles,
                body.owner.expect("validated owner"),
            );
            continue;
        }
        if body.kind == 3 {
            let visual = body.prop.as_ref().expect("validated prop");
            let prop = crate::scenery::Prop {
                kind: visual.kind,
                variant: visual.variant,
                scale: visual.scale,
                position: Vec2::new(body.position[0], body.position[2]),
                yaw: 0., // Root already carries the authoritative rotation.
            };
            commands.entity(entity).with_children(|parent| {
                for (mesh, material, transform) in crate::scenery::parts(&assets.forest, &prop) {
                    parent.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        transform,
                    ));
                }
            });
            continue;
        }
        let mesh = match body.kind {
            4 => assets.fireball.clone(),
            2 => assets.ring.clone(),
            _ => assets.cube.clone(),
        };
        commands.entity(entity).insert((
            Mesh3d(mesh),
            MeshMaterial3d(assets.materials[usize::from(body.kind.min(4))].clone()),
            body_transform(body),
        ));
    }
}
fn body_transform(body: &crate::network::Body) -> Transform {
    if let Some(prop) = &body.prop {
        return Transform {
            translation: Vec3::new(body.position[0], 0., body.position[2]),
            rotation: Quat::from_array(body.rotation),
            scale: Vec3::splat(prop.scale),
        };
    }
    Transform {
        translation: Vec3::from_array(body.position),
        rotation: Quat::from_array(body.rotation),
        scale: if body.kind == 1 {
            Vec3::splat(0.65)
        } else {
            Vec3::from_array(body.scale)
        },
    }
}

#[cfg(test)]
mod visual_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn vertex_bounds(mesh: &Mesh, transform: &Transform) -> (Vec3, Vec3) {
        use bevy::mesh::VertexAttributeValues;
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("rendered mesh has no positions");
        };
        assert!(!vertices.is_empty());
        vertices.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), v| {
                let point = transform.transform_point(Vec3::from_array(*v));
                (min.min(point), max.max(point))
            },
        )
    }

    #[test]
    fn replicated_fireball_mesh_matches_source_world_bounds_through_its_lifecycle() {
        use crate::explosions::{self, Fireball};
        for chain in 0..3 {
            for posed in [false, true] {
                let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
                explosions::replication_test_burst(sim.app.world_mut(), chain);
                let mut app = App::new();
                app.init_resource::<Assets<Mesh>>()
                    .init_resource::<Assets<StandardMaterial>>()
                    .insert_resource(View {
                        built: true,
                        ..default()
                    })
                    .add_systems(Update, render_snapshot);
                // A nearby client prop must survive: rendering cannot wreck scenery.
                let sentinel = app
                    .world_mut()
                    .spawn((
                        crate::scenery::Scenery {
                            hit_radius: 1.,
                            height: 2.,
                        },
                        Transform::from_xyz(100., 2., -100.),
                    ))
                    .id();
                let mut identities = std::collections::HashMap::new();
                for progress in [0.0, 0.5, 0.95] {
                    explosions::replication_test_progress(sim.app.world_mut(), progress);
                    if posed {
                        let world = sim.app.world_mut();
                        for mut transform in world
                            .query_filtered::<&mut Transform, With<Fireball>>()
                            .iter_mut(world)
                        {
                            transform.translation += Vec3::new(0.3, -0.2, 0.7);
                            transform.rotation = Quat::from_euler(EulerRot::XYZ, 0.3, 0.7, -0.4);
                            transform.scale *= Vec3::new(0.6, 1.4, 2.1);
                        }
                    }
                    // Use the real authoritative serializer, then the real replica spawner.
                    let mut frame = sim.network_frame();
                    frame.bodies.retain(|body| body.kind == 4);
                    let balls: Vec<_> = frame
                        .bodies
                        .iter()
                        .filter(|b| b.kind == 4)
                        .cloned()
                        .collect();
                    assert!(
                        !balls.is_empty(),
                        "chain {chain} must produce actual fireballs"
                    );
                    app.world_mut().resource_mut::<View>().frame = Some(frame);
                    app.update();
                    let source = sim.app.world_mut();
                    let source_balls: Vec<_> = source
                        .query_filtered::<(&Mesh3d, &Transform), With<Fireball>>()
                        .iter(source)
                        .map(|(mesh, transform)| (mesh.0.clone(), *transform))
                        .collect();
                    let world = app.world_mut();
                    let replicas: Vec<_> = world
                        .query::<(Entity, &Replica, &Mesh3d, &Transform)>()
                        .iter(world)
                        .map(|(entity, id, mesh, transform)| {
                            (entity, id.0, mesh.0.clone(), *transform)
                        })
                        .collect();
                    assert_eq!(replicas.len(), balls.len());
                    for body in &balls {
                        let (entity, _, mesh, transform) =
                            replicas.iter().find(|r| r.1 == body.id).unwrap();
                        if let Some(previous) = identities.insert(body.id, *entity) {
                            assert_eq!(previous, *entity, "stable ID must update in place");
                        }
                        let (source_mesh, source_transform) = source_balls
                            .iter()
                            .find(|(_, t)| t.translation.to_array() == body.position)
                            .unwrap();
                        let expected = vertex_bounds(
                            source.resource::<Assets<Mesh>>().get(source_mesh).unwrap(),
                            source_transform,
                        );
                        let actual = vertex_bounds(
                            world.resource::<Assets<Mesh>>().get(mesh).unwrap(),
                            transform,
                        );
                        assert!(
                            actual.0.abs_diff_eq(expected.0, 0.0001)
                                && actual.1.abs_diff_eq(expected.1, 0.0001),
                            "chain={chain} posed={posed} progress={progress}: source bounds {expected:?}, replica bounds {actual:?}"
                        );
                    }
                    for _ in 0..3 {
                        app.update();
                    }
                    let world = app.world_mut();
                    assert!(
                        world.get_entity(sentinel).is_ok(),
                        "client destroyed scenery"
                    );
                    assert_eq!(
                        world.query::<&explosions::Detonation>().iter(world).count(),
                        0
                    );
                    assert_eq!(world.query::<&Fireball>().iter(world).count(), 0);
                    assert_eq!(world.query::<&explosions::Debris>().iter(world).count(), 0);
                }
                explosions::replication_test_progress(sim.app.world_mut(), 1.01);
                let mut frame = sim.network_frame();
                frame.bodies.retain(|body| body.kind == 4);
                assert!(frame.bodies.iter().all(|b| b.kind != 4));
                app.world_mut().resource_mut::<View>().frame = Some(frame);
                app.update();
                let world = app.world_mut();
                assert_eq!(world.query::<&Replica>().iter(world).count(), 0);
                assert!(world.get_entity(sentinel).is_ok());
            }
        }
    }

    #[test]
    fn pickup_replica_geometry_stays_unchanged() {
        let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
        let mut frame = sim.network_frame();
        frame.bodies = (1..3)
            .map(|kind| crate::network::Body {
                id: u64::from(kind) + 1,
                kind,
                position: [3., 2., -4.],
                rotation: Quat::from_rotation_y(0.7).to_array(),
                scale: [1.2, 0.8, 1.5],
                owner: (kind == 0).then_some(0),
                prop: None,
            })
            .collect();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(View {
                built: true,
                frame: Some(frame.clone()),
                ..default()
            })
            .add_systems(Update, render_snapshot);
        app.update();
        let world = app.world_mut();
        let replicas: Vec<_> = world
            .query::<(&Replica, &Mesh3d, &Transform)>()
            .iter(world)
            .map(|(id, mesh, transform)| (id.0, mesh.0.clone(), *transform))
            .collect();
        assert_eq!(replicas.len(), 2);
        for body in &frame.bodies {
            let (_, handle, transform) = replicas.iter().find(|r| r.0 == body.id).unwrap();
            let expected: Mesh = match body.kind {
                0 => Sphere::new(0.16).mesh().ico(1).unwrap(),
                1 => Cuboid::new(1., 1., 1.).into(),
                2 => Torus::new(0.28, 0.5).into(),
                _ => unreachable!(),
            };
            let actual = world.resource::<Assets<Mesh>>().get(handle).unwrap();
            assert_eq!(
                actual.attribute(Mesh::ATTRIBUTE_POSITION),
                expected.attribute(Mesh::ATTRIBUTE_POSITION)
            );
            assert_eq!(
                actual.indices().map(|i| i.iter().collect::<Vec<_>>()),
                expected.indices().map(|i| i.iter().collect::<Vec<_>>())
            );
            let expected_transform = Transform {
                translation: Vec3::from_array(body.position),
                rotation: Quat::from_array(body.rotation),
                scale: if body.kind == 1 {
                    Vec3::splat(0.65)
                } else {
                    Vec3::from_array(body.scale)
                },
            };
            assert_eq!(
                vertex_bounds(actual, transform),
                vertex_bounds(&expected, &expected_transform)
            );
        }
    }

    #[test]
    fn hud_publication_is_atomic_and_legacy_status_has_a_safe_fallback() {
        let client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        let mut writes = Vec::new();
        assert!(publish_hud(&client, true, |key, value| {
            writes.push((key.to_owned(), value.to_owned()));
            true
        }));
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, "ryggattackHud");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&writes[0].1).unwrap(),
            hud_snapshot(&client, true)
        );
        assert!(!publish_hud(&client, false, |_, _| false));
        let failure = "Backend unavailable — reconnecting";
        assert_eq!(canvas_status_text(failure.into(), false), failure);
        assert_eq!(canvas_status_text(failure.into(), true), "");
        assert_eq!(canvas_status_text(failure.into(), false), failure);
    }
    #[test]
    fn hud_projection_is_a_small_whitelist_and_does_not_mutate_authority() {
        let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        client.room = Some(1);
        client.participant = Some(10);
        client.seat = Some(0);
        let mut lobby =
            crate::rooms::Lobby::new(1, "abcdefabcdefabcdefab".into(), 10, "Ada".into());
        lobby.phase = crate::rooms::Phase::Running;
        client.lobby = Some(lobby);
        let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
        let mut frame = sim.network_frame();
        frame.tick = 41;
        frame.simulation_tick = 37;
        frame.names[0] = Some("<Ada>".into());
        frame.wins = [3, 0, 0, 0];
        frame.winner = Some(0);
        frame.round_finished = true;
        frame.transition = 1.25;
        frame.carts[0].speed_seconds = 2.5;
        frame.carts[0].all_hits_seconds = 4.25;
        client.frame = Some(frame);
        client.status = "Online".into();
        client.notice = "public notice".into();
        client.rtt_ms = Some(12.5);
        let before = serde_json::to_value(client.frame.as_ref().unwrap()).unwrap();
        let value = hud_snapshot(&client, true);
        let expected = serde_json::json!({
            "lobby":{"id":1,"phase":"Running"},
            "frame":{
                "tick":41,"simulation_tick":37,"names":["<Ada>",null,null,null],
                "wins":[3,0,0,0],"winner":0,"round_finished":true,"transition":1.25,
                "carts":client.frame.as_ref().unwrap().carts.iter().map(|cart| serde_json::json!({
                    "id":cart.id,"active":cart.active,"alive":cart.alive,
                    "speed_seconds":cart.speed_seconds,"all_hits_seconds":cart.all_hits_seconds
                })).collect::<Vec<_>>()
            },
            "status":"Online","notice":"public notice","seat":0,"menu":true,"rtt_ms":12.5
        });
        assert_eq!(
            value, expected,
            "HUD must whitelist fields rather than serialize full snapshots"
        );
        assert!(
            !value.to_string().contains("abcdefabcdefabcdefab"),
            "private room canary leaked"
        );
        assert_eq!(
            serde_json::to_value(client.frame.as_ref().unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn hud_frame_is_quarantined_at_phase_and_identity_boundaries() {
        for boundary in [
            "lobby",
            "countdown",
            "no lobby",
            "room",
            "participant",
            "seat",
            "no seat",
            "leave",
        ] {
            let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
            client.room = Some(1);
            client.participant = Some(10);
            client.seat = Some(0);
            let mut lobby = crate::rooms::Lobby::new(1, "a".repeat(20), 10, "Ada".into());
            lobby.phase = crate::rooms::Phase::Running;
            client.lobby = Some(lobby);
            let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
            let mut frame = sim.network_frame();
            frame.winner = Some(0);
            frame.wins[0] = 3;
            frame.round_finished = true;
            frame.carts[0].speed_seconds = 5.;
            frame.carts[0].all_hits_seconds = 6.;
            client.frame = Some(frame);
            assert!(!hud_snapshot(&client, false)["frame"].is_null());
            match boundary {
                "lobby" => client.lobby.as_mut().unwrap().phase = crate::rooms::Phase::Lobby,
                "countdown" => {
                    client.lobby.as_mut().unwrap().phase =
                        crate::rooms::Phase::Countdown { ticks_left: 32 }
                }
                "no lobby" => client.lobby = None,
                "room" => client.room = Some(2),
                "participant" => client.participant = Some(11),
                "seat" => client.seat = Some(1),
                "no seat" => client.seat = None,
                "leave" => client.leave(),
                _ => unreachable!(),
            }
            let snapshot = hud_snapshot(&client, false);
            assert!(
                snapshot["frame"].is_null(),
                "{boundary} leaked old winner/powerups: {snapshot}"
            );
            if !matches!(boundary, "lobby" | "countdown") {
                assert!(
                    snapshot["lobby"].is_null(),
                    "{boundary}: lobby must agree with seat and room"
                );
                assert!(snapshot["seat"].is_null(), "{boundary}: no orphan seat");
            }
        }
    }
    #[test]
    fn lobby_keys_use_server_identity_and_explicit_commands() {
        let lobby = crate::rooms::Lobby::new(1, "0123456789abcdef0123".into(), 10, "Ada".into());
        assert!(matches!(
            lobby_key(KeyCode::KeyR, &lobby, 10),
            Some(crate::network::ClientMessage::Ready(true))
        ));
        assert!(
            matches!(lobby_key(KeyCode::KeyB,&lobby,10), Some(crate::network::ClientMessage::Configure(s)) if s.bots==0)
        );
        assert!(lobby_key(KeyCode::KeyB, &lobby, 11).is_none());
        assert!(
            matches!(lobby_key(KeyCode::KeyV,&lobby,10), Some(crate::network::ClientMessage::Configure(s)) if s.arena==crate::headless::Arena::Volcano)
        );
    }
    #[test]
    fn invalid_invite_is_visible_in_the_entry_ui_instead_of_exiting_before_hud() {
        let client = initial_client("ws://127.0.0.1:1", "Ada", Some("bad"));
        assert!(client.status.contains("invalid room code"));
        assert!(client.seat.is_none());
    }
    #[test]
    fn native_room_switch_preserves_ground_and_removes_old_cart_roots() {
        let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        client.leave();
        let mut app = App::new();
        app.insert_non_send(client)
            .insert_resource(Entry {
                address: "ws://127.0.0.1:1".into(),
                name: "Ada".into(),
                code: "0123456789abcdef0123".into(),
            })
            .init_resource::<View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, setup)
            .add_systems(Update, online_controls);
        app.update();
        let world = app.world_mut();
        // Environment floors are now created from a validated lobby. A marked
        // persistent floor must still be excluded from room-root cleanup.
        let ground = world.spawn((Ground, Mesh3d(Handle::default()))).id();
        let cart = world
            .spawn((
                crate::players::Player {
                    id: 0,
                    active: true,
                    alive: true,
                    score: 0,
                    shot_cooldown: 0.,
                    wants_to_fire: false,
                    input: None,
                },
                Transform::default(),
                Visibility::default(),
            ))
            .with_children(|p| {
                p.spawn((Mesh3d(Handle::default()), Transform::default()));
            })
            .id();
        let prop = world
            .spawn((Replica(71), Transform::default(), Visibility::default()))
            .with_children(|p| {
                p.spawn((Mesh3d(Handle::default()), Transform::default()));
            })
            .id();
        let prop_child = world.get::<Children>(prop).unwrap()[0];
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert!(
            app.world().get_entity(ground).is_ok(),
            "room switch erased the ground"
        );
        assert!(
            app.world().get_entity(cart).is_err(),
            "empty old cart roots accumulate across joins"
        );
        assert!(
            app.world().get_entity(prop).is_err(),
            "room switch leaked meshless prop root"
        );
        assert!(
            app.world().get_entity(prop_child).is_err(),
            "room switch leaked prop child"
        );
    }
    #[test]
    fn native_code_entry_creates_a_validated_join_adapter_after_deliberate_leave() {
        let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        client.leave();
        let mut app = App::new();
        app.insert_non_send(client)
            .insert_resource(Entry {
                address: "ws://127.0.0.1:1".into(),
                name: "Ada".into(),
                code: String::new(),
            })
            .init_resource::<View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, online_controls);
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyA);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .reset_all();
        }
        assert_eq!(app.world().resource::<Entry>().code, "a".repeat(20));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        assert_eq!(
            app.world().non_send::<NetworkClient>().status,
            "Connecting…"
        );
    }
    #[test]
    fn socket_polling_waits_until_initial_presentations_have_yielded() {
        let mut frames = 0;
        assert!(!presentation_ready(&mut frames));
        assert!(!presentation_ready(&mut frames));
        assert!(presentation_ready(&mut frames));
        assert!(presentation_ready(&mut frames));
    }
    #[test]
    fn menu_blur_and_form_focus_suppress_gameplay_without_pausing() {
        assert!(gameplay_active(false, true, true));
        assert!(!gameplay_active(true, true, true));
        assert!(!gameplay_active(false, false, true));
        assert!(!gameplay_active(false, true, false));
    }
    #[test]
    fn development_entry_requires_explicit_address_and_preserves_offline_default() {
        assert_eq!(native_entry(&[]), None);
        assert_eq!(
            native_entry(&[
                "--online".into(),
                "ws://localhost:8787".into(),
                "--name".into(),
                "Ada".into()
            ]),
            Some(("ws://localhost:8787".into(), "Ada".into()))
        );
    }
    #[test]
    fn lobby_boundary_removes_old_match_replicas_before_ids_can_restart() {
        let mut sim =
            crate::headless::Simulation::new(crate::headless::SimulationConfig::default()).unwrap();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(View {
                edges: Some(sim.network_edges()),
                frame: Some(sim.network_frame()),
                ..default()
            })
            .add_systems(Update, render_snapshot);
        app.update();
        app.update();
        let world = app.world_mut();
        assert!(world.query::<&Replica>().iter(world).count() > 0);
        world.resource_mut::<View>().frame = None;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.query::<&Replica>().iter(world).count(),
            0,
            "old match IDs/meshes survive lobby and can alias the next match"
        );
        assert!(
            world
                .query::<(&crate::players::Player, &Visibility)>()
                .iter(world)
                .all(|(_, v)| *v == Visibility::Hidden)
        );
    }
    #[test]
    fn client_only_renders_server_state_and_removes_destroyed_bodies() {
        let mut sim =
            crate::headless::Simulation::new(crate::headless::SimulationConfig::default()).unwrap();
        let frame = sim.network_frame();
        let prop = frame.bodies.iter().find(|b| b.kind == 3).unwrap().id;
        let position = Vec3::from_array(frame.carts[0].position);
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(View {
                edges: Some(sim.network_edges()),
                frame: Some(frame),
                ..default()
            })
            .add_systems(Update, render_snapshot);
        app.update();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<(&crate::players::Player, &Transform)>()
                .iter(world)
                .find(|(c, _)| c.id == 0)
                .unwrap()
                .1
                .translation,
            position
        );
        assert!(world.query::<&Replica>().iter(world).any(|r| r.0 == prop));
        world
            .resource_mut::<View>()
            .frame
            .as_mut()
            .unwrap()
            .bodies
            .retain(|b| b.id != prop);
        app.update();
        let world = app.world_mut();
        assert!(!world.query::<&Replica>().iter(world).any(|r| r.0 == prop));
        for _ in 0..10 {
            app.update();
        }
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<(&crate::players::Player, &Transform)>()
                .iter(world)
                .find(|(c, _)| c.id == 0)
                .unwrap()
                .1
                .translation,
            position,
            "render frames must not simulate movement"
        );
    }
}
