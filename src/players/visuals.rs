//! Procedural cart and rider models and their initial placement.

use bevy::prelude::*;

use super::{PLAYER_COLORS, Player};
use crate::tracks::{RailFollower, RailMap, RailPath, track_pose};

const SKIN_COLORS: [Color; 4] = [
    Color::srgb(1.00, 0.72, 0.55),
    Color::srgb(0.93, 0.62, 0.46),
    Color::srgb(0.55, 0.31, 0.20),
    Color::srgb(1.00, 0.78, 0.60),
];

const HAIR_COLORS: [Color; 4] = [
    Color::srgb(0.30, 0.12, 0.045),
    Color::srgb(0.22, 0.07, 0.035),
    Color::srgb(0.13, 0.055, 0.025),
    Color::srgb(0.95, 0.58, 0.10),
];

pub(crate) fn spawn_players(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rail_map: &RailMap,
) {
    let cart_body_mesh = meshes.add(Cuboid::new(1.0, 0.36, 1.28));
    let cart_cabin_mesh = meshes.add(Cuboid::new(0.88, 0.28, 0.76));
    let bumper_mesh = meshes.add(Cuboid::new(1.08, 0.13, 0.13));
    let side_rail_mesh = meshes.add(Cuboid::new(0.08, 0.28, 0.82));
    let wheel_mesh = meshes.add(Cylinder::new(0.21, 0.16));
    let seat_mesh = meshes.add(Cuboid::new(0.62, 0.48, 0.11));
    let torso_mesh = meshes.add(Cuboid::new(0.43, 0.46, 0.29));
    let head_mesh = meshes.add(Sphere::new(0.25));
    let hair_mesh = meshes.add(Sphere::new(0.275));
    let hat_crown_mesh = meshes.add(Cylinder::new(0.24, 0.15));
    let hat_brim_mesh = meshes.add(Cuboid::new(0.58, 0.055, 0.42));
    let arm_mesh = meshes.add(Cuboid::new(0.115, 0.38, 0.115));
    let hand_mesh = meshes.add(Sphere::new(0.09));
    let control_bar_mesh = meshes.add(Cuboid::new(0.66, 0.07, 0.07));
    let eye_mesh = meshes.add(Sphere::new(0.038));
    let target_mesh = meshes.add(Cuboid::new(0.58, 0.52, 0.10));

    let metal_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.67, 0.72),
        metallic: 0.75,
        perceptual_roughness: 0.28,
        ..default()
    });
    let wheel_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.055, 0.06, 0.075),
        metallic: 0.35,
        perceptual_roughness: 0.52,
        ..default()
    });
    let seat_material = materials.add(Color::srgb(0.22, 0.11, 0.055));
    let eye_material = materials.add(Color::srgb(0.025, 0.03, 0.045));
    let control_material = materials.add(Color::srgb(0.16, 0.08, 0.035));
    let target_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.12, 0.12),
        emissive: LinearRgba::rgb(1.5, 0.05, 0.05),
        ..default()
    });

    for id in 0..4 {
        let (from, to) = rail_map.starting_route(id);
        let path = RailPath::Edge { from, to };
        let (position, direction) = track_pose(path, 0.0);
        let mut transform = Transform::from_translation(position);
        transform.look_at(position + direction, Vec3::Y);
        let player_material = materials.add(StandardMaterial {
            base_color: PLAYER_COLORS[id],
            metallic: 0.22,
            perceptual_roughness: 0.38,
            ..default()
        });
        let skin_material = materials.add(SKIN_COLORS[id]);
        let hair_material = materials.add(HAIR_COLORS[id]);

        commands
            .spawn((
                Player {
                    id,
                    score: 0,
                    shot_cooldown: id as f32 * 0.12,
                    wants_to_fire: false,
                    input: None,
                },
                RailFollower {
                    path,
                    progress: 0.0,
                    requested_turn: Vec3::ZERO,
                    collision_cooldown: 0.0,
                },
                transform,
                Visibility::default(),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(cart_body_mesh.clone()),
                    MeshMaterial3d(player_material.clone()),
                    Transform::default(),
                ));
                parent.spawn((
                    Mesh3d(cart_cabin_mesh.clone()),
                    MeshMaterial3d(player_material.clone()),
                    Transform::from_xyz(0.0, 0.27, 0.08),
                ));
                parent.spawn((
                    Mesh3d(bumper_mesh.clone()),
                    MeshMaterial3d(metal_material.clone()),
                    Transform::from_xyz(0.0, -0.02, -0.69),
                ));
                for side in [-1.0, 1.0] {
                    parent.spawn((
                        Mesh3d(side_rail_mesh.clone()),
                        MeshMaterial3d(metal_material.clone()),
                        Transform::from_xyz(side * 0.48, 0.31, 0.08),
                    ));
                    for z in [-0.39, 0.39] {
                        parent.spawn((
                            Mesh3d(wheel_mesh.clone()),
                            MeshMaterial3d(wheel_material.clone()),
                            Transform::from_xyz(side * 0.55, -0.08, z)
                                .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                        ));
                    }
                }

                // The driver sits behind the controls and faces local -Z,
                // which is also the cart's forward direction.
                parent.spawn((
                    Mesh3d(seat_mesh.clone()),
                    MeshMaterial3d(seat_material.clone()),
                    Transform::from_xyz(0.0, 0.62, 0.31),
                ));
                parent.spawn((
                    Mesh3d(torso_mesh.clone()),
                    MeshMaterial3d(player_material.clone()),
                    Transform::from_xyz(0.0, 0.72, 0.04),
                ));
                parent.spawn((
                    Mesh3d(hair_mesh.clone()),
                    MeshMaterial3d(hair_material.clone()),
                    Transform::from_xyz(0.0, 1.12, 0.015),
                ));
                parent.spawn((
                    Mesh3d(head_mesh.clone()),
                    MeshMaterial3d(skin_material.clone()),
                    Transform::from_xyz(0.0, 1.11, -0.055),
                ));
                parent.spawn((
                    Mesh3d(hat_brim_mesh.clone()),
                    MeshMaterial3d(player_material.clone()),
                    Transform::from_xyz(0.0, 1.34, -0.07),
                ));
                parent.spawn((
                    Mesh3d(hat_crown_mesh.clone()),
                    MeshMaterial3d(player_material.clone()),
                    Transform::from_xyz(0.0, 1.43, 0.0),
                ));
                for side in [-1.0, 1.0] {
                    parent.spawn((
                        Mesh3d(arm_mesh.clone()),
                        MeshMaterial3d(player_material.clone()),
                        Transform::from_xyz(side * 0.25, 0.72, -0.15)
                            .with_rotation(Quat::from_rotation_x(-0.72)),
                    ));
                    parent.spawn((
                        Mesh3d(hand_mesh.clone()),
                        MeshMaterial3d(skin_material.clone()),
                        Transform::from_xyz(side * 0.25, 0.56, -0.33),
                    ));
                    parent.spawn((
                        Mesh3d(eye_mesh.clone()),
                        MeshMaterial3d(eye_material.clone()),
                        Transform::from_xyz(side * 0.085, 1.15, -0.285),
                    ));
                }
                parent.spawn((
                    Mesh3d(control_bar_mesh.clone()),
                    MeshMaterial3d(control_material.clone()),
                    Transform::from_xyz(0.0, 0.53, -0.39),
                ));
                parent.spawn((
                    Mesh3d(target_mesh.clone()),
                    MeshMaterial3d(target_material.clone()),
                    Transform::from_xyz(0.0, 0.18, 0.68),
                ));
            });
    }
}
