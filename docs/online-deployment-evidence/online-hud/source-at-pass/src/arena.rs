//! Arena visuals; the railway and combat rules are identical in both themes.
use crate::match_settings::{Arena, MatchSettings};
use crate::scenery::{Forest, Transient, rebuild_forest};
use crate::tracks::RailMap;
use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct ArenaDecoration;
#[derive(Component)]
pub(crate) struct Ground(pub(crate) bool);

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_arena(
    mut commands: Commands,
    settings: Res<MatchSettings>,
    mut forest: ResMut<Forest>,
    map: Res<RailMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    transient: Query<Entity, With<Transient>>,
    decorations: Query<Entity, With<ArenaDecoration>>,
    grounds: Query<(&Ground, &MeshMaterial3d<StandardMaterial>)>,
    mut lights: Query<&mut DirectionalLight>,
    mut ambient: Query<&mut AmbientLight>,
    mut fogs: Query<&mut DistanceFog>,
    mut clear: ResMut<ClearColor>,
    mut applied: Local<Option<Arena>>,
) {
    if *applied == Some(settings.arena) {
        return;
    }
    let volcanic = settings.arena == Arena::Volcano;
    forest.set_arena(settings.arena, &mut materials);
    rebuild_forest(&mut commands, &forest, &transient);
    for entity in &decorations {
        commands.entity(entity).despawn();
    }
    for (ground, handle) in &grounds {
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.base_color = match (volcanic, ground.0) {
                (true, true) => Color::srgb(0.22, 0.19, 0.23),
                (true, false) => Color::srgb(0.11, 0.09, 0.13),
                (false, true) => Color::srgb(0.27, 0.46, 0.19),
                (false, false) => Color::srgb(0.17, 0.35, 0.14),
            };
        }
    }
    clear.0 = if volcanic {
        Color::srgb(0.30, 0.16, 0.19)
    } else {
        Color::srgb(0.62, 0.72, 0.66)
    };
    for mut fog in &mut fogs {
        fog.color = clear.0;
    }
    for mut light in &mut lights {
        light.color = if volcanic {
            Color::srgb(1.0, 0.72, 0.54)
        } else {
            Color::srgb(1.0, 0.95, 0.85)
        };
    }
    for mut light in &mut ambient {
        light.color = if volcanic {
            Color::srgb(0.75, 0.48, 0.45)
        } else {
            Color::srgb(0.58, 0.70, 0.60)
        };
    }
    if volcanic {
        let lava = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.23, 0.015),
            emissive: LinearRgba::new(2.8, 0.36, 0.01, 1.0),
            unlit: true,
            ..default()
        });
        let basalt = materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.10, 0.14),
            perceptual_roughness: 1.0,
            ..default()
        });
        let pool = meshes.add(Cylinder::new(1.0, 0.02).mesh().resolution(24));
        let river = meshes.add(Cuboid::new(1.0, 0.02, 1.0));
        // The river is outside the walls. It cannot conceal a cart or rail.
        for segment in 0..26 {
            let z = 12.0 - segment as f32 * 1.6;
            let x = -12.0 - (z * 0.22).sin() * 1.5;
            commands.spawn((
                ArenaDecoration,
                Mesh3d(river.clone()),
                MeshMaterial3d(lava.clone()),
                Transform::from_xyz(x, 0.005, z).with_scale(Vec3::new(2.5, 1.0, 2.3)),
            ));
        }
        // Small pools only occupy gaps between rails, flush with the ground.
        for prop in forest
            .props
            .iter()
            .filter(|p| {
                p.position.abs().max_element() < 8.0 && map.distance_to_track(p.position) > 1.2
            })
            .take(18)
        {
            commands.spawn((
                ArenaDecoration,
                Mesh3d(pool.clone()),
                MeshMaterial3d(lava.clone()),
                Transform::from_xyz(prop.position.x, 0.018, prop.position.y)
                    .with_scale(Vec3::new(0.65, 1.0, 0.45)),
            ));
        }
        let mountain = meshes.add(
            ConicalFrustum {
                radius_top: 0.9,
                radius_bottom: 4.0,
                height: 4.0,
            }
            .mesh()
            .resolution(24),
        );
        // A compact right-flank crater stays in the 16:9 camera frame. Its
        // entire base clears the wall; behind/right of play it cannot hide carts.
        for (x, z, scale) in [(-15.0, -17.0, 1.0), (1.5, -21.0, 1.4), (13.0, -5.0, 0.8)] {
            commands.spawn((
                ArenaDecoration,
                Mesh3d(mountain.clone()),
                MeshMaterial3d(basalt.clone()),
                Transform::from_xyz(x, 2.0 * scale, z).with_scale(Vec3::splat(scale)),
            ));
            commands.spawn((
                ArenaDecoration,
                Mesh3d(pool.clone()),
                MeshMaterial3d(lava.clone()),
                Transform::from_xyz(x, 4.0 * scale + 0.01, z).with_scale(Vec3::new(
                    0.9 * scale,
                    1.0,
                    0.9 * scale,
                )),
            ));
        }
    }
    *applied = Some(settings.arena);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::CameraProjection;
    #[test]
    fn a_crater_is_in_frame_while_volcano_footprints_stay_outside_rails() {
        let mut app = App::new();
        let map = RailMap::random();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let forest = Forest::new(&mut meshes, &mut materials, &map);
        app.insert_resource(meshes)
            .insert_resource(materials)
            .insert_resource(map)
            .insert_resource(forest)
            .insert_resource(MatchSettings {
                arena: Arena::Volcano,
                ..default()
            })
            .insert_resource(ClearColor(Color::WHITE));
        app.add_systems(Update, apply_arena);
        app.update();
        let mountains: Vec<Transform> = app
            .world_mut()
            .query_filtered::<&Transform, With<ArenaDecoration>>()
            .iter(app.world())
            .filter(|t| t.translation.y > 1.0 && t.scale.x == t.scale.y && t.scale.y == t.scale.z)
            .copied()
            .collect();
        assert_eq!(mountains.len(), 3);
        let camera = Transform::from_xyz(0.0, 16.5, 15.5).looking_at(Vec3::ZERO, Vec3::Y);
        let view = camera.to_matrix().inverse();
        let projection = PerspectiveProjection {
            aspect_ratio: 1280.0 / 720.0,
            ..default()
        }
        .get_clip_from_view();
        let mut visible = false;
        for mountain in mountains {
            let radius = 4.0 * mountain.scale.x;
            let position = mountain.translation;
            assert!(
                position.x.abs() - radius > crate::scene::ARENA_HALF_SIZE
                    || position.z.abs() - radius > crate::scene::ARENA_HALF_SIZE,
                "volcano footprint overlaps the arena/rails: {mountain:?}"
            );
            // All four edges of a glowing crater must fit, not just a sliver of base.
            let crater = Vec3::new(position.x, 4.0 * mountain.scale.x + 0.01, position.z);
            let rim = 0.9 * mountain.scale.x;
            visible |= [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
                .iter()
                .all(|axis| {
                    let clip = projection * view * (crater + *axis * rim).extend(1.0);
                    let ndc = clip.xyz() / clip.w;
                    clip.w > 0.0 && ndc.x.abs() < 0.95 && ndc.y.abs() < 0.95
                });
        }
        assert!(
            visible,
            "no complete volcano crater in the 1280x720 camera frame"
        );
    }
    #[test]
    fn volcano_selection_builds_a_distinct_environment() {
        let mut app = App::new();
        let map = RailMap::random();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let forest = Forest::new(&mut meshes, &mut materials, &map);
        let ground_material = materials.add(StandardMaterial::default());
        let ground = app
            .world_mut()
            .spawn((Ground(true), MeshMaterial3d(ground_material.clone())))
            .id();
        app.insert_resource(meshes)
            .insert_resource(materials)
            .insert_resource(map)
            .insert_resource(forest);
        app.insert_resource(MatchSettings {
            arena: Arena::Volcano,
            ..default()
        })
        .insert_resource(ClearColor(Color::WHITE))
        .insert_resource(crate::settings::VideoSettings::default());
        app.add_systems(
            Update,
            (apply_arena, crate::scenery::apply_forest_density).chain(),
        );
        app.update();
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<ArenaDecoration>>()
                .iter(app.world())
                .count()
                > 10
        );
        assert_ne!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&ground_material)
                .unwrap()
                .base_color,
            Color::WHITE
        );
        assert!(app.world().get_entity(ground).is_ok());
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<ArenaDecoration>>()
            .iter(app.world())
            .count();
        app.world_mut()
            .resource_mut::<crate::settings::VideoSettings>()
            .forest = crate::settings::ForestDensity::Sparse;
        app.update();
        assert_eq!(app.world().resource::<Forest>().arena, Arena::Volcano);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ArenaDecoration>>()
                .iter(app.world())
                .count(),
            count
        );
        assert!(
            app.world_mut()
                .query::<&crate::scenery::Scenery>()
                .iter(app.world())
                .all(|s| s.height <= 0.83)
        );
        let transforms: Vec<Transform> = app
            .world_mut()
            .query_filtered::<&Transform, With<ArenaDecoration>>()
            .iter(app.world())
            .copied()
            .collect();
        let map = app.world().resource::<RailMap>();
        for transform in transforms {
            if transform.translation.x.abs() < 9.0 && transform.translation.z.abs() < 9.0 {
                assert!(map.distance_to_track(transform.translation.xz()) > 1.2);
                assert!(transform.translation.y < 0.03);
            }
        }
        app.world_mut().resource_mut::<MatchSettings>().arena = Arena::Forest;
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ArenaDecoration>>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&ground_material)
                .unwrap()
                .base_color,
            Color::srgb(0.27, 0.46, 0.19)
        );
        assert!(
            app.world_mut()
                .query::<&crate::scenery::Scenery>()
                .iter(app.world())
                .any(|s| s.height > 1.0)
        );
    }
}
