//! A presentation-only online app: intentionally does not install any gameplay,
//! local pause, scoring, RNG, collision, or round-reset system.
use crate::{input::ControlIntent, net_client::NetworkClient, network::Frame};
use bevy::prelude::*;
#[derive(Resource, Default)]
struct View {
    edges: Option<Vec<(usize, usize)>>,
    frame: Option<Frame>,
    previous: Option<Frame>,
    alpha: f32,
    built: bool,
}
#[derive(Component)]
struct Replica(u64);
#[derive(Component)]
struct Status;
struct VisualAssets {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
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
pub(crate) fn run(address: String, name: String) {
    let client = match NetworkClient::connect(&address, &name) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Online: {err}");
            return;
        }
    };
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.06, 0.10, 0.12)))
        .insert_non_send(client)
        .init_resource::<View>()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Ryggattack — Online development match".into(),
                resolution: (1100, 720).into(),
                canvas: Some("#ryggattack-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (network_input, render_snapshot).chain())
        .run();
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0., 19., 16.).looking_at(Vec3::ZERO, Vec3::Y),
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
        Mesh3d(meshes.add(Cuboid::new(19., 0.2, 19.))),
        MeshMaterial3d(materials.add(Color::srgb(0.15, 0.23, 0.18))),
        Transform::from_xyz(0., -0.2, 0.),
    ));
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
    let active = gameplay_active(*menu, windows.iter().all(|w| w.focused), canvas_focused);
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
    if view.edges.is_none() {
        view.edges = client.edges.clone();
    }
    view.previous = client.previous.clone();
    view.frame = client.frame.clone();
    // One snapshot interval of visual interpolation, no extrapolation/prediction.
    view.alpha = (client.received.elapsed().as_secs_f32() / 0.0625).clamp(0., 1.);
    let detail=client.frame.as_ref().map(|f|format!("tick {}  wins {:?}  winner {:?}\nPlayers: {:?}\nPowerups: speed {:.1}s / all-hits {:.1}s; drop {:.1}s",f.tick,f.wins,f.winner,f.names,client.seat.map_or(0.,|s|f.carts[s].speed_seconds),client.seat.map_or(0.,|s|f.carts[s].all_hits_seconds),f.until_drop)).unwrap_or_default();
    let label = format!(
        "{}  ack RTT {:.0}ms\n{}\nWASD/arrows: junction • Space: fire • Esc: {} (server keeps running)",
        client.status,
        client.rtt_ms.unwrap_or(0.),
        detail,
        if *menu {
            "resume input"
        } else {
            "suppress input"
        }
    );
    for mut text in &mut status {
        **text = label.clone();
    }
    #[cfg(target_arch = "wasm32")]
    {
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
#[allow(clippy::too_many_arguments)]
fn render_snapshot(
    mut commands: Commands,
    mut view: ResMut<View>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut carts: Query<(&mut crate::players::Player, &mut Transform, &mut Visibility)>,
    mut replicas: Query<(Entity, &Replica, &mut Transform), Without<crate::players::Player>>,
    mut assets: Local<Option<VisualAssets>>,
) {
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
        cube: meshes.add(Cuboid::new(1., 1., 1.)),
        sphere: meshes.add(Sphere::new(0.16).mesh().ico(1).unwrap()),
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
    let Some(frame) = &view.frame else {
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
        let mesh = match body.kind {
            0 | 4 => assets.sphere.clone(),
            2 => assets.ring.clone(),
            _ => assets.cube.clone(),
        };
        commands.spawn((
            Replica(body.id),
            Mesh3d(mesh),
            MeshMaterial3d(assets.materials[usize::from(body.kind.min(4))].clone()),
            body_transform(body),
        ));
    }
}
fn body_transform(body: &crate::network::Body) -> Transform {
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
mod tests {
    use super::*;
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
