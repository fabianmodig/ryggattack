//! Startup scene assembly: camera, lighting, ground, tracks, forest, carts, and HUD.

use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::combat::MissileAssets;
use crate::explosions::{EffectAssets, EffectRng};
use crate::players::spawn_players;
use crate::scenery::{Forest, spawn_forest};
use crate::settings::{Shadows, VideoSettings, WorldCamera};
use crate::tracks::{RailMap, SimpleRng, spawn_tracks};
use crate::ui::spawn_hud;

pub(crate) const ARENA_HALF_SIZE: f32 = 9.0;

/// The woods run to the edge of the frame, and the mist takes over from there.
const GROUND_SIZE: f32 = 100.0;

/// The far edge of the frame is about forty units out, so the fog closes in
/// just past it: what would be popping trees is a misty tree line instead.
const FOG_COLOR: Color = Color::srgb(0.62, 0.72, 0.66);

/// Shadows reach the same forty units, and no further: past that the woods
/// are mist. One cascade at this size covers it, where Bevy's default of four
/// cascades out to a hundred and fifty units rendered the whole forest four
/// times a frame, which is what made an integrated GPU crawl. The size of the
/// shadow map is chosen by the automatic quality.
const SHADOW_DISTANCE: f32 = 40.0;

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rail_map: Res<RailMap>,
    settings: Res<VideoSettings>,
    match_settings: Res<crate::match_settings::MatchSettings>,
) {
    info!("Generated railway map with seed {}", rail_map.seed);

    commands.spawn((
        Camera3d::default(),
        WorldCamera,
        // Until the quality moves the world into its own lowered-resolution
        // picture, this camera draws straight into the canvas, UI included.
        IsDefaultUiCamera,
        if settings.anti_aliasing {
            Msaa::Sample4
        } else {
            Msaa::Off
        },
        Transform::from_xyz(0.0, 16.5, 15.5).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog {
            color: FOG_COLOR,
            falloff: FogFalloff::Linear {
                start: 24.0,
                end: 48.0,
            },
            ..default()
        },
    ));

    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.85),
            illuminance: 9_000.0,
            shadow_maps_enabled: settings.shadows != Shadows::Off,
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: SHADOW_DISTANCE,
            ..default()
        }
        .build(),
        Transform::from_xyz(4.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn(AmbientLight {
        color: Color::srgb(0.58, 0.70, 0.60),
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });

    for (index, (mesh, material, transform)) in floor_and_walls(&mut meshes, &mut materials)
        .into_iter()
        .enumerate()
    {
        let mut entity = commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform));
        if index < 2 {
            entity.insert((crate::arena::Ground(index == 0), NotShadowCaster));
        }
    }
    spawn_tracks(&mut commands, &mut meshes, &mut materials, &rail_map);

    setup_gameplay(
        &mut commands,
        &mut meshes,
        &mut materials,
        &rail_map,
        match_settings.arena,
    );
    spawn_hud(&mut commands);
}

/// Shared presentation geometry; the first two pieces are the inner/outer ground.
pub(crate) fn floor_and_walls(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mut parts = Vec::new();
    for (color, roughness, transform) in [
        (
            Color::srgb(0.27, 0.46, 0.19),
            0.95,
            Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(
                ARENA_HALF_SIZE * 2.0,
                0.2,
                ARENA_HALF_SIZE * 2.0,
            )),
        ),
        (
            Color::srgb(0.17, 0.35, 0.14),
            1.0,
            Transform::from_xyz(0.0, -0.12, -8.0).with_scale(Vec3::new(
                GROUND_SIZE,
                0.2,
                GROUND_SIZE,
            )),
        ),
    ] {
        parts.push((
            cube.clone(),
            materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: roughness,
                ..default()
            }),
            transform,
        ));
    }
    let wall = materials.add(StandardMaterial {
        base_color: Color::srgb(0.44, 0.29, 0.15),
        perceptual_roughness: 0.85,
        ..default()
    });
    for (position, scale) in [
        (
            Vec3::new(0.0, 0.35, -ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0 + 0.5, 0.7, 0.35),
        ),
        (
            Vec3::new(0.0, 0.35, ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0 + 0.5, 0.7, 0.35),
        ),
        (
            Vec3::new(-ARENA_HALF_SIZE, 0.35, 0.0),
            Vec3::new(0.35, 0.7, ARENA_HALF_SIZE * 2.0 + 0.5),
        ),
        (
            Vec3::new(ARENA_HALF_SIZE, 0.35, 0.0),
            Vec3::new(0.35, 0.7, ARENA_HALF_SIZE * 2.0 + 0.5),
        ),
    ] {
        parts.push((
            cube.clone(),
            wall.clone(),
            Transform::from_translation(position).with_scale(scale),
        ));
    }
    parts
}

/// CPU-side gameplay world, also used without render/window plugins. Visual
/// asset construction remains here during the first extraction milestone.
pub(crate) fn setup_gameplay(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rail_map: &RailMap,
    arena: crate::match_settings::Arena,
) {
    let mut forest = Forest::new(meshes, materials, rail_map);
    forest.set_arena(arena, materials);
    spawn_forest(commands, &forest);
    commands.insert_resource(forest);
    commands.insert_resource(EffectAssets::new(meshes, materials));
    commands.insert_resource(EffectRng(SimpleRng::new(rail_map.seed.rotate_left(17))));
    commands.insert_resource(MissileAssets::new(meshes, materials));
    spawn_players(commands, meshes, materials, rail_map);
}
