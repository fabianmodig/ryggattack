use super::*;
use crate::net_client::NetworkClient;
use crate::online::{Replica, render_snapshot, sync_visual_boundary};
use crate::rooms::{Lobby, Phase};

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<View>()
        .insert_resource(ClearColor(Color::BLACK))
        .add_systems(Update, (render_snapshot, present).chain());
    app
}

fn client(arena: Arena) -> (NetworkClient, crate::headless::Simulation) {
    let mut simulation =
        crate::headless::Simulation::new(crate::headless::SimulationConfig { arena, ..default() })
            .unwrap();
    let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
    client.room = Some(1);
    client.participant = Some(10);
    client.seat = Some(0);
    let mut lobby = Lobby::new(1, "a".repeat(20), 10, "Ada".into());
    lobby.phase = Phase::Running;
    lobby.settings.arena = arena;
    client.lobby = Some(lobby);
    client.edges = Some(simulation.network_edges());
    client.frame = Some(simulation.network_frame());
    (client, simulation)
}

fn update(app: &mut App, client: &NetworkClient) {
    sync_visual_boundary(client, &mut app.world_mut().resource_mut::<View>());
    app.update();
    app.update(); // Deferred cart creation must also consume the snapshot.
    let world = app.world_mut();
    assert_eq!(
        world
            .query::<&crate::scenery::Scenery>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(
        world
            .query::<&crate::scenery::Transient>()
            .iter(world)
            .count(),
        0
    );
    assert!(!world.contains_resource::<crate::scenery::Forest>());
    assert!(!world.contains_resource::<RailMap>());
}

fn environment(app: &mut App) -> Option<Entity> {
    let roots: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<Environment>>()
        .iter(app.world())
        .collect();
    assert!(roots.len() <= 1, "duplicate environment roots");
    roots.first().copied()
}

fn children(app: &App, entity: Entity) -> Vec<Entity> {
    app.world()
        .get::<Children>(entity)
        .unwrap()
        .iter()
        .collect()
}

fn assets(app: &App) -> (usize, usize) {
    (
        app.world().resource::<Assets<Mesh>>().len(),
        app.world().resource::<Assets<StandardMaterial>>().len(),
    )
}

fn replicas(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Replica>>()
        .iter(app.world())
        .collect()
}

#[test]
fn authoritative_forest_volcano_forest_and_round_boundaries_reuse_cached_assets() {
    let (mut client, _) = client(Arena::Forest);
    let mut app = app();
    update(&mut app, &client);
    let first = environment(&mut app).unwrap();
    let forest_children = children(&app, first);
    let counts = assets(&app);
    for _ in 0..3 {
        for arena in [Arena::Volcano, Arena::Forest] {
            let (_, mut source) = self::client(arena);
            client.lobby.as_mut().unwrap().settings.arena = arena;
            client.visual_epoch += 1; // Coalesced lobby/countdown/new Running.
            client.frame = Some(source.network_frame());
            let old = environment(&mut app).unwrap();
            let old_children = children(&app, old);
            let old_replicas = replicas(&mut app);
            update(&mut app, &client);
            assert!(app.world().get_entity(old).is_err());
            assert!(
                old_children
                    .iter()
                    .chain(old_replicas.iter())
                    .all(|e| app.world().get_entity(*e).is_err())
            );
            let current = environment(&mut app).unwrap();
            assert_eq!(assets(&app), counts, "arena change allocated new assets");
            let pieces = children(&app, current);
            let floor = app
                .world()
                .get::<MeshMaterial3d<StandardMaterial>>(pieces[0])
                .unwrap();
            assert_eq!(
                app.world()
                    .resource::<Assets<StandardMaterial>>()
                    .get(&floor.0)
                    .unwrap()
                    .base_color,
                if arena == Arena::Volcano {
                    Color::srgb(0.22, 0.19, 0.23)
                } else {
                    Color::srgb(0.27, 0.46, 0.19)
                }
            );
            if arena == Arena::Forest {
                assert_eq!(pieces.len(), forest_children.len());
            }
            if arena == Arena::Volcano {
                // The only green scene materials would be stale Forest foliage.
                for entity in &pieces {
                    let handle = &app
                        .world()
                        .get::<MeshMaterial3d<StandardMaterial>>(*entity)
                        .unwrap()
                        .0;
                    let color = app
                        .world()
                        .resource::<Assets<StandardMaterial>>()
                        .get(handle)
                        .unwrap()
                        .base_color
                        .to_srgba();
                    assert!(
                        color.green <= color.red,
                        "Forest decoration survived in Volcano"
                    );
                }
                let glowing = pieces
                    .iter()
                    .filter(|entity| {
                        let handle = &app
                            .world()
                            .get::<MeshMaterial3d<StandardMaterial>>(**entity)
                            .unwrap()
                            .0;
                        app.world()
                            .resource::<Assets<StandardMaterial>>()
                            .get(handle)
                            .unwrap()
                            .unlit
                    })
                    .count();
                assert!(glowing >= 29, "missing lava river or crater rims");
            }
            client.lobby.as_mut().unwrap().phase = Phase::Lobby;
            update(&mut app, &client);
            assert_eq!(environment(&mut app), Some(current));
            assert!(replicas(&mut app).is_empty());
            client.lobby.as_mut().unwrap().phase = Phase::Countdown { ticks_left: 1 };
            update(&mut app, &client);
            assert_eq!(environment(&mut app), Some(current));
            client.lobby.as_mut().unwrap().phase = Phase::Running;
            update(&mut app, &client);
            assert_eq!(environment(&mut app), Some(current));
            assert_eq!(assets(&app), counts);
        }
    }
}

#[test]
fn reconnect_another_room_leave_rejoin_and_reload_have_no_stale_decorations_or_id_aliases() {
    let (mut client, _) = client(Arena::Volcano);
    let mut app = app();
    update(&mut app, &client);
    let counts = assets(&app);
    let original = environment(&mut app).unwrap();
    let old_replicas = replicas(&mut app);
    client.visual_epoch += 1;
    update(&mut app, &client);
    assert_eq!(
        environment(&mut app),
        Some(original),
        "reconnect churned static scenery"
    );
    assert!(
        old_replicas
            .iter()
            .all(|e| app.world().get_entity(*e).is_err())
    );
    client.room = Some(2);
    client.lobby.as_mut().unwrap().id = 2;
    update(&mut app, &client);
    assert!(app.world().get_entity(original).is_err());
    assert_eq!(assets(&app), counts);
    let root = environment(&mut app).unwrap();
    let old_children = children(&app, root);
    client.leave();
    update(&mut app, &client);
    assert!(environment(&mut app).is_none());
    assert!(
        old_children
            .iter()
            .all(|e| app.world().get_entity(*e).is_err())
    );
    assert!(replicas(&mut app).is_empty());
    let (next, _) = self::client(Arena::Forest);
    update(&mut app, &next);
    assert_eq!(assets(&app), counts);
    let mut reload = self::app();
    update(&mut reload, &next);
    assert_eq!(assets(&reload), assets(&app));
    let a = environment(&mut app).unwrap();
    let b = environment(&mut reload).unwrap();
    assert_eq!(children(&app, a).len(), children(&reload, b).len());
    for (left, right) in children(&app, a).into_iter().zip(children(&reload, b)) {
        assert_eq!(
            app.world().get::<Transform>(left),
            reload.world().get::<Transform>(right)
        );
        let mesh = |app: &App, entity| {
            let handle = &app.world().get::<Mesh3d>(entity).unwrap().0;
            app.world()
                .resource::<Assets<Mesh>>()
                .get(handle)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .clone()
        };
        assert_eq!(mesh(&app, left), mesh(&reload, right));
    }
    // Invalid room/seat data must not apply an untrusted arena selection.
    let mut invalid = next;
    invalid.lobby.as_mut().unwrap().id = 999;
    update(&mut app, &invalid);
    assert!(environment(&mut app).is_none());
}

#[test]
fn source_volcano_decorations_keep_rails_and_camera_clear_and_sparse_backdrop_stays_unreachable() {
    let (_, simulation) = client(Arena::Volcano);
    let map = RailMap::from_edges(0, simulation.network_edges());
    let props = layout(&map);
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let parts = crate::arena::volcano_parts(&mut meshes, &mut materials, &map, &props);
    assert!(parts.len() >= 32); // River, all three basalt craters and glowing rims.
    for (_, _, t) in &parts {
        if t.translation.x.abs() < 9.0 && t.translation.z.abs() < 9.0 {
            assert!(map.distance_to_track(t.translation.xz()) > 1.2);
            assert!(t.translation.y < 0.03);
        } else if t.translation.y > 1.0 {
            let radius = if t.scale.x == t.scale.y {
                4.0 * t.scale.x
            } else {
                t.scale.x
            };
            assert!(t.translation.x.abs() - radius > 9.0 || t.translation.z.abs() - radius > 9.0);
        }
    }
    assert!(props.iter().filter(|p| !p.can_be_wrecked()).count() > 100);
    let forest = ForestAssets::new(&mut meshes, &mut materials);
    let sparse = backdrop_meshes(
        &mut meshes,
        &forest,
        &props,
        Arena::Forest,
        ForestDensity::Sparse,
    );
    assert!(sparse.len() > 6, "mixed woodland materials/patches missing");
}

#[test]
fn a_different_authoritative_topology_replaces_rails_carts_and_environment_once() {
    let (mut client, _) = client(Arena::Forest);
    let mut app = app();
    update(&mut app, &client);
    let root = environment(&mut app).unwrap();
    let old_children = children(&app, root);
    let old_carts: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<crate::players::Player>>()
        .iter(app.world())
        .collect();
    let mut next = crate::headless::Simulation::new(crate::headless::SimulationConfig {
        seed: 77,
        arena: Arena::Volcano,
        ..default()
    })
    .unwrap();
    assert_ne!(client.edges.as_ref().unwrap(), &next.network_edges());
    client.room = Some(2);
    client.lobby.as_mut().unwrap().id = 2;
    client.lobby.as_mut().unwrap().settings.arena = Arena::Volcano;
    client.edges = Some(next.network_edges());
    client.frame = Some(next.network_frame());
    update(&mut app, &client);
    assert!(
        old_children
            .iter()
            .chain(old_carts.iter())
            .all(|e| app.world().get_entity(*e).is_err())
    );
    assert!(app.world().get_entity(root).is_err());
    assert_eq!(
        app.world().resource::<View>().edges.as_ref().unwrap(),
        &next.network_edges()
    );
    assert_eq!(
        app.world_mut()
            .query::<&crate::players::Player>()
            .iter(app.world())
            .count(),
        4
    );
    let stable = environment(&mut app);
    let counts = assets(&app);
    for _ in 0..12 {
        update(&mut app, &client);
    }
    assert_eq!(environment(&mut app), stable);
    assert_eq!(assets(&app), counts);
}

#[test]
fn coalesced_same_theme_round_keeps_background_but_replaces_authoritative_ids() {
    let (mut client, _) = client(Arena::Forest);
    let mut app = app();
    update(&mut app, &client);
    let root = environment(&mut app);
    let old = replicas(&mut app);
    let counts = assets(&app);
    client.visual_epoch += 1;
    update(&mut app, &client);
    assert_eq!(environment(&mut app), root);
    assert_eq!(assets(&app), counts);
    assert!(old.iter().all(|e| app.world().get_entity(*e).is_err()));
    assert_eq!(replicas(&mut app).len(), old.len());
}

#[test]
fn real_native_leave_create_controls_reuse_models_and_cache_without_material_churn() {
    let (mut client, _) = client(Arena::Forest);
    let mut app = app();
    update(&mut app, &client);
    let counts = assets(&app);
    let carts: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<crate::players::Player>>()
        .iter(app.world())
        .collect();
    app.insert_resource(crate::online::Entry {
        address: "ws://127.0.0.1:1".into(),
        name: "Ada".into(),
        code: String::new(),
    })
    .init_resource::<ButtonInput<KeyCode>>()
    .add_systems(
        Update,
        crate::online::online_controls.before(render_snapshot),
    );
    for arena in [Arena::Volcano, Arena::Forest, Arena::Volcano] {
        client.leave();
        sync_visual_boundary(&client, &mut app.world_mut().resource_mut::<View>());
        app.insert_non_send(client);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F2);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(environment(&mut app).is_none());
        assert!(replicas(&mut app).is_empty());
        assert!(carts.iter().all(|e| app.world().get_entity(*e).is_ok()));
        assert!(
            carts
                .iter()
                .all(|e| *app.world().get::<Visibility>(*e).unwrap() == Visibility::Hidden)
        );
        assert_eq!(assets(&app), counts);
        let (next, _) = self::client(arena);
        client = next;
        update(&mut app, &client);
        assert!(environment(&mut app).is_some());
        assert_eq!(assets(&app), counts);
    }
}
