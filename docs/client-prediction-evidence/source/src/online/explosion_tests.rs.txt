//! Presentation-only regression: real source burst -> wire -> replica renderer.
use super::*;
use crate::{explosions, headless::Simulation, settings::VideoSettings};

fn source(chain: usize) -> Simulation {
    let mut sim = Simulation::new(Default::default()).unwrap();
    explosions::replication_test_burst(sim.app.world_mut(), chain);
    sim
}
fn frame(sim: &mut Simulation) -> Frame {
    let mut frame = sim.network_frame();
    frame.bodies.retain(|body| body.kind == 4);
    serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap()
}
fn replica(frame: Frame, level: usize) -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .insert_resource(VideoSettings::level(level))
        .insert_resource(View {
            built: true,
            frame: Some(frame),
            ..default()
        })
        .add_systems(Update, render_snapshot);
    app.update();
    app
}
fn count<T: Component>(world: &mut World) -> usize {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .count()
}

#[test]
fn every_source_material_stage_matches_replica_without_asset_growth() {
    for chain in 0..3 {
        let mut sim = source(chain);
        let mut app = replica(frame(&mut sim), 6);
        let assets = (
            app.world().resource::<Assets<Mesh>>().len(),
            app.world().resource::<Assets<StandardMaterial>>().len(),
        );
        for progress in [0.0, 0.17, 0.34, 0.50, 0.67, 0.84, 0.99] {
            explosions::replication_test_progress(sim.app.world_mut(), progress);
            let frame = frame(&mut sim);
            assert_eq!(frame.bodies.len(), 3);
            assert_eq!(
                frame
                    .bodies
                    .iter()
                    .filter(|b| b.explosion.unwrap().burst == Some(chain))
                    .count(),
                1
            );
            app.world_mut().resource_mut::<View>().frame = Some(frame.clone());
            app.update();
            let source = sim.app.world_mut();
            let source_balls: Vec<_> = source.query_filtered::<(&Transform, &MeshMaterial3d<StandardMaterial>), With<explosions::Fireball>>()
                .iter(source).map(|(t, m)| (t.translation, m.0.clone())).collect();
            let world = app.world_mut();
            let copies: Vec<_> = world
                .query::<(
                    &Replica,
                    &MeshMaterial3d<StandardMaterial>,
                    &bevy::light::NotShadowCaster,
                )>()
                .iter(world)
                .map(|(id, material, _)| (id.0, material.0.clone()))
                .collect();
            assert_eq!(copies.len(), 3);
            for body in &frame.bodies {
                let (_, original) = source_balls
                    .iter()
                    .find(|(p, _)| p.to_array() == body.position)
                    .unwrap();
                let (_, copy) = copies.iter().find(|(id, _)| *id == body.id).unwrap();
                let a = source
                    .resource::<Assets<StandardMaterial>>()
                    .get(original)
                    .unwrap();
                let b = world
                    .resource::<Assets<StandardMaterial>>()
                    .get(copy)
                    .unwrap();
                assert_eq!(
                    a.base_color, b.base_color,
                    "chain={chain} progress={progress}"
                );
                assert_eq!(a.alpha_mode, b.alpha_mode);
                assert_eq!(a.unlit, b.unlit);
                assert_eq!(a.emissive, b.emissive);
            }
            assert_eq!(
                (
                    world.resource::<Assets<Mesh>>().len(),
                    world.resource::<Assets<StandardMaterial>>().len()
                ),
                assets
            );
            assert_eq!(count::<explosions::Detonation>(world), 0);
            assert_eq!(count::<crate::combat::Projectile>(world), 0);
        }
    }
}

#[test]
fn auxiliary_effects_obey_quality_and_do_not_retrigger_or_survive_boundaries() {
    for level in [0, 2, 6] {
        for chain in 0..3 {
            let mut sim = source(chain);
            let initial = frame(&mut sim);
            let mut app = replica(initial.clone(), level);
            let budget = VideoSettings::level(level).effects.budget();
            let size = if chain == 0 { 1.0 } else { 0.55 };
            for _ in 0..5 {
                let world = app.world_mut();
                assert_eq!(count::<Replica>(world), budget.fireballs);
                assert_eq!(count::<explosions::Smoke>(world), budget.smoke_puffs);
                assert_eq!(
                    count::<explosions::Debris>(world),
                    (budget.sparks * size) as usize
                );
                assert_eq!(
                    count::<explosions::Flash>(world),
                    usize::from(budget.max_flashes > 0)
                );
                assert_eq!(count::<explosions::Scorch>(world), usize::from(chain == 0));
                assert_eq!(count::<explosions::Detonation>(world), 0);
                app.update();
            }
            // Lobby clears even the light-only roots and long-lived ground marks.
            app.world_mut().resource_mut::<View>().frame = None;
            app.update();
            assert_eq!(count::<crate::scenery::Transient>(app.world_mut()), 0);
            assert_eq!(count::<Replica>(app.world_mut()), 0);
            // A new match may reuse IDs, but gets only one fresh burst.
            app.world_mut().resource_mut::<View>().frame = Some(initial);
            app.world_mut().resource_mut::<View>().reset_replicas = true;
            app.update();
            assert_eq!(
                count::<explosions::Smoke>(app.world_mut()),
                budget.smoke_puffs
            );
            app.world_mut().resource_mut::<View>().reset_scene = true;
            app.world_mut().resource_mut::<View>().reset_replicas = true;
            app.world_mut().resource_mut::<View>().frame = None;
            app.update();
            assert_eq!(count::<crate::scenery::Transient>(app.world_mut()), 0);
        }
    }
}

#[test]
fn older_server_without_presentation_metadata_still_renders_and_removes_balls() {
    let mut sim = source(0);
    let mut old = serde_json::to_value(frame(&mut sim)).unwrap();
    for body in old["bodies"].as_array_mut().unwrap() {
        body.as_object_mut().unwrap().remove("explosion");
    }
    let old: Frame = serde_json::from_value(old).unwrap();
    assert!(old.bodies.iter().all(|body| body.explosion.is_none()));
    let mut app = replica(old, 0);
    assert_eq!(count::<Replica>(app.world_mut()), 3);
    assert_eq!(count::<crate::scenery::Transient>(app.world_mut()), 0);
    app.world_mut().resource_mut::<View>().frame = None;
    app.update();
    assert_eq!(count::<Replica>(app.world_mut()), 0);
}
