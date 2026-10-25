//! CPU-side source -> real serializer -> real replica hierarchy assertions.
use super::*;
use crate::scenery::{Forest, Prop, PropKind, Scenery};
use bevy::ecs::system::RunSystemOnce;
use bevy::mesh::VertexAttributeValues;

fn app(frame: Frame, arena: crate::match_settings::Arena) -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(View {
            built: true,
            frame: Some(frame),
            arena,
            ..default()
        })
        .add_systems(Update, render_snapshot);
    app.update();
    app
}

fn roots(world: &mut World) -> std::collections::HashMap<u64, Entity> {
    world
        .query::<(Entity, &Replica)>()
        .iter(world)
        .map(|(e, r)| (r.0, e))
        .collect()
}

fn children(world: &World, root: Entity) -> Vec<Entity> {
    world.get::<Children>(root).unwrap().iter().collect()
}

fn bounds(mesh: &Mesh, matrix: Mat4) -> (Vec3, Vec3) {
    let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("missing vertices")
    };
    vertices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(lo, hi), v| {
            let p = matrix.transform_point3(Vec3::from_array(*v));
            (lo.min(p), hi.max(p))
        },
    )
}

fn compare_parts(source: &World, root: Entity, replica: &World, copy: Entity, count: usize) {
    let left = children(source, root);
    let right = children(replica, copy);
    assert_eq!(left.len(), count);
    assert_eq!(right.len(), count);
    let placement = source.get::<Transform>(root).unwrap().to_matrix();
    let replica_placement = replica.get::<Transform>(copy).unwrap().to_matrix();
    for (a, b) in left.into_iter().zip(right) {
        let at = source.get::<Transform>(a).unwrap();
        let bt = replica.get::<Transform>(b).unwrap();
        assert_eq!(at, bt, "local model part placement");
        let am = source
            .resource::<Assets<Mesh>>()
            .get(&source.get::<Mesh3d>(a).unwrap().0)
            .unwrap();
        let bm = replica
            .resource::<Assets<Mesh>>()
            .get(&replica.get::<Mesh3d>(b).unwrap().0)
            .unwrap();
        assert_eq!(
            am.attribute(Mesh::ATTRIBUTE_POSITION),
            bm.attribute(Mesh::ATTRIBUTE_POSITION)
        );
        assert_eq!(
            am.indices().map(|i| i.iter().collect::<Vec<_>>()),
            bm.indices().map(|i| i.iter().collect::<Vec<_>>())
        );
        let expected = bounds(am, placement * at.to_matrix());
        let actual = bounds(bm, replica_placement * bt.to_matrix());
        assert!(
            actual.0.abs_diff_eq(expected.0, 0.0001) && actual.1.abs_diff_eq(expected.1, 0.0001),
            "part bounds: {expected:?} != {actual:?}"
        );
        let a = source
            .resource::<Assets<StandardMaterial>>()
            .get(&source.get::<MeshMaterial3d<StandardMaterial>>(a).unwrap().0)
            .unwrap();
        let b = replica
            .resource::<Assets<StandardMaterial>>()
            .get(
                &replica
                    .get::<MeshMaterial3d<StandardMaterial>>(b)
                    .unwrap()
                    .0,
            )
            .unwrap();
        assert_eq!(a.base_color, b.base_color);
        assert_eq!(a.emissive, b.emissive);
        assert_eq!(a.metallic, b.metallic);
        assert_eq!(a.perceptual_roughness, b.perceptual_roughness);
        assert_eq!(a.unlit, b.unlit);
    }
}

fn no_gameplay(world: &mut World) {
    assert_eq!(
        world
            .query::<&crate::combat::Projectile>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(world.query::<&Scenery>().iter(world).count(), 0);
    assert_eq!(
        world
            .query::<&crate::explosions::Detonation>()
            .iter(world)
            .count(),
        0
    );
    assert_eq!(
        world
            .query::<&crate::explosions::Fireball>()
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
}

#[test]
fn all_effective_prop_parts_match_source_across_variants_scales_yaws_and_volcano_cap() {
    for arena in [
        crate::match_settings::Arena::Forest,
        crate::match_settings::Arena::Volcano,
    ] {
        let mut sim = crate::headless::Simulation::new(crate::headless::SimulationConfig {
            arena,
            ..default()
        })
        .unwrap();
        let world = sim.app.world_mut();
        let old: Vec<_> = world
            .query_filtered::<Entity, With<Scenery>>()
            .iter(world)
            .collect();
        for e in old {
            world.despawn(e);
        }
        let mut props = Vec::new();
        for kind in [
            PropKind::Pine,
            PropKind::Birch,
            PropKind::Bush,
            PropKind::Rock,
            PropKind::Stump,
            PropKind::Fern,
        ] {
            for variant in 0..2 {
                for scale in [0.75, 1.15, 1.5, 2.2] {
                    for yaw in [0., 0.7, -2.4] {
                        props.push(Prop {
                            kind,
                            variant,
                            scale,
                            yaw,
                            position: Vec2::new(props.len() as f32 * 0.02 - 4., 2.),
                        });
                    }
                }
            }
        }
        world.resource_mut::<Forest>().props = props.clone();
        world
            .run_system_once(|mut commands: Commands, forest: Res<Forest>| {
                crate::scenery::spawn_forest(&mut commands, &forest)
            })
            .unwrap();
        let frame: Frame =
            serde_json::from_str(&serde_json::to_string(&sim.network_frame()).unwrap()).unwrap();
        assert_eq!(frame.bodies.len(), props.len());
        let source_roots: Vec<_> = sim
            .app
            .world_mut()
            .query_filtered::<(Entity, &Prop), With<Scenery>>()
            .iter(sim.app.world())
            .map(|(e, p)| (e, *p))
            .collect();
        let mut app = app(frame.clone(), arena);
        let asset_counts = (
            app.world().resource::<Assets<Mesh>>().len(),
            app.world().resource::<Assets<StandardMaterial>>().len(),
        );
        let original = roots(app.world_mut());
        for body in &frame.bodies {
            let (source, effective) = source_roots
                .iter()
                .find(|(_, p)| p.position.x == body.position[0])
                .unwrap();
            let requested = props
                .iter()
                .find(|p| p.position == effective.position)
                .unwrap();
            let visual = body.prop.as_ref().unwrap();
            assert_eq!(visual.variant, requested.variant);
            assert_eq!(
                visual.kind,
                if arena == crate::match_settings::Arena::Volcano {
                    PropKind::Rock
                } else {
                    requested.kind
                }
            );
            assert_eq!(
                visual.scale,
                if arena == crate::match_settings::Arena::Volcano {
                    requested.scale.min(1.5)
                } else {
                    requested.scale
                }
            );
            let count = match visual.kind {
                PropKind::Pine | PropKind::Fern => 4,
                PropKind::Birch | PropKind::Bush => 3,
                _ => 2,
            };
            compare_parts(
                sim.app.world(),
                *source,
                app.world(),
                original[&body.id],
                count,
            );
            assert!(
                app.world().get::<Mesh3d>(original[&body.id]).is_none(),
                "not a collider cube"
            );
        }
        let original_children: Vec<_> = original
            .values()
            .flat_map(|e| children(app.world(), *e))
            .collect();
        // Wire collider dimensions/centre are not art dimensions. Even unrelated
        // collider envelopes must not stretch or raise the visual model.
        let mut moved = frame.clone();
        for body in &mut moved.bodies {
            body.position[1] = 99.;
            body.scale = [7., 8., 9.];
            body.position[0] += 0.25;
        }
        app.world_mut().resource_mut::<View>().frame = Some(moved);
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            roots(app.world_mut()),
            original,
            "stable roots update in place"
        );
        for body in &frame.bodies {
            let t = app.world().get::<Transform>(original[&body.id]).unwrap();
            assert_eq!(
                t.translation,
                Vec3::new(body.position[0] + 0.25, 0., body.position[2])
            );
            assert_eq!(t.scale, Vec3::splat(body.prop.as_ref().unwrap().scale));
        }
        assert!(
            original_children
                .iter()
                .all(|e| app.world().get_entity(*e).is_ok())
        );
        no_gameplay(app.world_mut());
        assert_eq!(
            asset_counts,
            (
                app.world().resource::<Assets<Mesh>>().len(),
                app.world().resource::<Assets<StandardMaterial>>().len()
            )
        );
        app.world_mut()
            .resource_mut::<View>()
            .frame
            .as_mut()
            .unwrap()
            .bodies
            .clear();
        app.update();
        assert!(roots(app.world_mut()).is_empty());
        assert!(
            original_children
                .iter()
                .all(|e| app.world().get_entity(*e).is_err())
        );
    }
}

#[test]
fn missiles_match_source_hierarchy_forward_owner_paint_and_glow_without_simulating() {
    let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
    let world = sim.app.world_mut();
    for (mut player, mut transform) in world
        .query::<(&mut crate::players::Player, &mut Transform)>()
        .iter_mut(world)
    {
        player.active = true;
        player.alive = true;
        player.wants_to_fire = true;
        player.shot_cooldown = 0.;
        *transform = Transform::from_xyz(player.id as f32, 1., -2.)
            .with_rotation(Quat::from_rotation_y(player.id as f32 * 1.2));
    }
    world
        .run_system_once(crate::combat::fire_projectiles)
        .unwrap();
    let shots: Vec<_> = world
        .query_filtered::<(Entity, &crate::combat::Projectile), With<Children>>()
        .iter(world)
        .map(|(e, p)| (e, p.owner))
        .collect();
    assert_eq!(shots.len(), 4);
    for (entity, owner) in &shots {
        let mut t = world.get_mut::<Transform>(*entity).unwrap();
        t.rotation *= Quat::from_rotation_x(0.2);
        t.scale = Vec3::new(0.8, 1.2, 1.5);
        let parts = children(world, *entity);
        let nose = world.get::<Transform>(parts[1]).unwrap().translation;
        let exhaust = world.get::<Transform>(parts[4]).unwrap().translation;
        let t = world.get::<Transform>(*entity).unwrap();
        assert!((t.rotation * nose).dot(t.forward().as_vec3()) > 0.);
        assert!((t.rotation * exhaust).dot(t.forward().as_vec3()) < 0.);
        for i in [0, 4] {
            let material = world
                .resource::<Assets<StandardMaterial>>()
                .get(
                    &world
                        .get::<MeshMaterial3d<StandardMaterial>>(parts[i])
                        .unwrap()
                        .0,
                )
                .unwrap();
            assert_eq!(material.base_color, crate::players::PLAYER_COLORS[*owner]);
        }
    }
    let mut frame = sim.network_frame();
    frame.bodies.retain(|b| b.kind == 0);
    let frame: Frame = serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap();
    let mut app = app(frame.clone(), Default::default());
    let original = roots(app.world_mut());
    for body in &frame.bodies {
        let (source, _) = shots
            .iter()
            .find(|(_, owner)| Some(*owner) == body.owner)
            .unwrap();
        compare_parts(sim.app.world(), *source, app.world(), original[&body.id], 5);
    }
    let parts: Vec<_> = original
        .values()
        .flat_map(|e| children(app.world(), *e))
        .collect();
    let mut moved = frame.clone();
    for body in &mut moved.bodies {
        body.position[2] -= 0.7;
        body.rotation = (Quat::from_rotation_y(0.45) * Quat::from_array(body.rotation)).to_array();
        body.scale = [1.3, 0.7, 0.9];
    }
    app.world_mut().resource_mut::<View>().frame = Some(moved.clone());
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(roots(app.world_mut()), original);
    for body in &moved.bodies {
        assert_eq!(
            *app.world().get::<Transform>(original[&body.id]).unwrap(),
            body_transform(body)
        );
    }
    assert!(parts.iter().all(|e| app.world().get_entity(*e).is_ok()));
    no_gameplay(app.world_mut());
    app.world_mut().resource_mut::<View>().frame = None;
    app.update();
    assert!(parts.iter().all(|e| app.world().get_entity(*e).is_err()));
    assert!(roots(app.world_mut()).is_empty());
}

#[test]
fn renderer_boundaries_replace_same_ids_and_remove_entire_child_hierarchies() {
    for boundary in [
        "lobby",
        "countdown",
        "leave",
        "room",
        "participant",
        "seat",
        "reconnect",
        "coalesced round",
    ] {
        let mut sim = crate::headless::Simulation::new(Default::default()).unwrap();
        let mut frame = sim.network_frame();
        frame.bodies.retain(|b| b.kind == 3);
        frame.bodies.truncate(1);
        let mut client = NetworkClient::connect("ws://127.0.0.1:1", "Ada").unwrap();
        client.room = Some(1);
        client.participant = Some(10);
        client.seat = Some(0);
        let mut lobby = crate::rooms::Lobby::new(1, "a".repeat(20), 10, "Ada".into());
        lobby.phase = crate::rooms::Phase::Running;
        client.lobby = Some(lobby);
        client.frame = Some(frame.clone());
        let mut app = app(frame.clone(), Default::default());
        sync_visual_boundary(&client, &mut app.world_mut().resource_mut::<View>());
        app.update();
        let original = roots(app.world_mut());
        let old_children: Vec<_> = original
            .values()
            .flat_map(|e| children(app.world(), *e))
            .collect();
        match boundary {
            "lobby" => client.lobby.as_mut().unwrap().phase = crate::rooms::Phase::Lobby,
            "countdown" => {
                client.lobby.as_mut().unwrap().phase =
                    crate::rooms::Phase::Countdown { ticks_left: 1 }
            }
            "leave" => client.leave(),
            "room" => client.room = Some(2),
            "participant" => client.participant = Some(11),
            "seat" => client.seat = Some(1),
            "reconnect" | "coalesced round" => client.visual_epoch += 1,
            _ => unreachable!(),
        }
        sync_visual_boundary(&client, &mut app.world_mut().resource_mut::<View>());
        app.update();
        assert!(
            old_children
                .iter()
                .all(|e| app.world().get_entity(*e).is_err()),
            "{boundary}: leaked children"
        );
        assert!(
            original
                .values()
                .all(|e| app.world().get_entity(*e).is_err()),
            "{boundary}: aliased old root"
        );
        let next = roots(app.world_mut());
        assert_eq!(
            next.len(),
            if matches!(boundary, "reconnect" | "coalesced round") {
                1
            } else {
                0
            }
        );
        // Fresh match may use exactly the same numeric IDs and different art.
        let mut body = frame.bodies[0].clone();
        body.kind = 0;
        body.prop = None;
        body.owner = Some(3);
        let view = &mut *app.world_mut().resource_mut::<View>();
        view.frame = Some(frame.clone());
        view.frame.as_mut().unwrap().bodies = vec![body];
        view.reset_replicas = true;
        app.update();
        app.update();
        let next = roots(app.world_mut());
        assert_eq!(next.len(), 1);
        assert_eq!(
            children(app.world(), *next.values().next().unwrap()).len(),
            5
        );
        no_gameplay(app.world_mut());
    }
}
