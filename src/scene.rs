//! Startup scene assembly: camera, lighting, arena, tracks, carts, and HUD.

use bevy::prelude::*;

use crate::players::spawn_players;
use crate::tracks::{RailMap, spawn_tracks};
use crate::ui::spawn_hud;

pub(crate) const ARENA_HALF_SIZE: f32 = 9.0;

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rail_map: Res<RailMap>,
) {
    info!("Generated railway map with seed {}", rail_map.seed);

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 16.5, 15.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn(AmbientLight {
        color: Color::srgb(0.55, 0.62, 0.78),
        brightness: 250.0,
        affects_lightmapped_meshes: true,
    });

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.13, 0.20),
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(
            ARENA_HALF_SIZE * 2.0,
            0.2,
            ARENA_HALF_SIZE * 2.0,
        ))),
        MeshMaterial3d(floor_material),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));

    spawn_tracks(&mut commands, &mut meshes, &mut materials, &rail_map);

    let wall_mesh = meshes.add(Cuboid::new(1.0, 0.7, 1.0));
    let wall_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.27, 0.38),
        metallic: 0.15,
        ..default()
    });
    for (position, scale) in [
        (
            Vec3::new(0.0, 0.35, -ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0 + 0.5, 1.0, 0.35),
        ),
        (
            Vec3::new(0.0, 0.35, ARENA_HALF_SIZE),
            Vec3::new(ARENA_HALF_SIZE * 2.0 + 0.5, 1.0, 0.35),
        ),
        (
            Vec3::new(-ARENA_HALF_SIZE, 0.35, 0.0),
            Vec3::new(0.35, 1.0, ARENA_HALF_SIZE * 2.0 + 0.5),
        ),
        (
            Vec3::new(ARENA_HALF_SIZE, 0.35, 0.0),
            Vec3::new(0.35, 1.0, ARENA_HALF_SIZE * 2.0 + 0.5),
        ),
    ] {
        commands.spawn((
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_material.clone()),
            Transform::from_translation(position).with_scale(scale),
        ));
    }

    spawn_players(&mut commands, &mut meshes, &mut materials, &rail_map);
    spawn_hud(&mut commands);
}
