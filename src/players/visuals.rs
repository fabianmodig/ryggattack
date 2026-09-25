//! Procedural cart and rider models and their initial placement.

use bevy::prelude::*;

use super::{PLAYER_COLORS, Player};
use crate::batch::MaterialBatches;
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

/// Bevy's default sphere has five subdivisions, 720 triangles, which is far
/// finer than a head or an eye a few pixels across can show. The outline of
/// these stays within a fraction of a pixel of it at the game's camera.
fn sphere(radius: f32, subdivisions: u32) -> Mesh {
    Sphere::new(radius)
        .mesh()
        .ico(subdivisions)
        .expect("a handful of subdivisions is well within the limit")
}

/// The meshes every cart and rider is assembled from.
struct CartMeshes {
    cart_body: Mesh,
    cart_cabin: Mesh,
    bumper: Mesh,
    side_rail: Mesh,
    wheel: Mesh,
    seat: Mesh,
    torso: Mesh,
    head: Mesh,
    hair: Mesh,
    hat_crown: Mesh,
    hat_brim: Mesh,
    arm: Mesh,
    hand: Mesh,
    control_bar: Mesh,
    eye: Mesh,
    target: Mesh,
}

impl CartMeshes {
    fn new() -> Self {
        Self {
            cart_body: Cuboid::new(1.0, 0.36, 1.28).into(),
            cart_cabin: Cuboid::new(0.88, 0.28, 0.76).into(),
            bumper: Cuboid::new(1.08, 0.13, 0.13).into(),
            side_rail: Cuboid::new(0.08, 0.28, 0.82).into(),
            wheel: Cylinder::new(0.21, 0.16).into(),
            seat: Cuboid::new(0.62, 0.48, 0.11).into(),
            torso: Cuboid::new(0.43, 0.46, 0.29).into(),
            head: sphere(0.25, 3),
            hair: sphere(0.275, 3),
            hat_crown: Cylinder::new(0.24, 0.15).into(),
            hat_brim: Cuboid::new(0.58, 0.055, 0.42).into(),
            arm: Cuboid::new(0.115, 0.38, 0.115).into(),
            hand: sphere(0.09, 2),
            control_bar: Cuboid::new(0.66, 0.07, 0.07).into(),
            eye: sphere(0.038, 2),
            target: Cuboid::new(0.58, 0.52, 0.10).into(),
        }
    }
}

struct CartMaterials {
    player: Handle<StandardMaterial>,
    skin: Handle<StandardMaterial>,
    hair: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    wheel: Handle<StandardMaterial>,
    seat: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    control: Handle<StandardMaterial>,
    target: Handle<StandardMaterial>,
}

pub(crate) fn spawn_players(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rail_map: &RailMap,
) {
    let parts = CartMeshes::new();

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
        let cart_materials = CartMaterials {
            player: materials.add(StandardMaterial {
                base_color: PLAYER_COLORS[id],
                metallic: 0.22,
                perceptual_roughness: 0.38,
                ..default()
            }),
            skin: materials.add(SKIN_COLORS[id]),
            hair: materials.add(HAIR_COLORS[id]),
            metal: metal_material.clone(),
            wheel: wheel_material.clone(),
            seat: seat_material.clone(),
            eye: eye_material.clone(),
            control: control_material.clone(),
            target: target_material.clone(),
        };

        // Every part of a cart and its rider is rigid relative to the cart,
        // so they are welded into one mesh per material: the same triangles,
        // but nine meshes a cart instead of twenty-two entities that are each
        // propagated, extracted, culled, and drawn (twice, with the shadow).
        let welded = cart_model(&parts, &cart_materials).build(meshes);

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
                for (mesh, material) in welded {
                    parent.spawn((Mesh3d(mesh), MeshMaterial3d(material), Transform::default()));
                }
            });
    }
}

/// A cart and its rider in the cart's own frame, which faces local -Z.
fn cart_model(parts: &CartMeshes, materials: &CartMaterials) -> MaterialBatches {
    let mut model = MaterialBatches::default();
    let mut add = |mesh: &Mesh, material: &Handle<StandardMaterial>, transform: Transform| {
        model.add(material, mesh, transform);
    };

    add(&parts.cart_body, &materials.player, Transform::default());
    add(
        &parts.cart_cabin,
        &materials.player,
        Transform::from_xyz(0.0, 0.27, 0.08),
    );
    add(
        &parts.bumper,
        &materials.metal,
        Transform::from_xyz(0.0, -0.02, -0.69),
    );
    for side in [-1.0, 1.0] {
        add(
            &parts.side_rail,
            &materials.metal,
            Transform::from_xyz(side * 0.48, 0.31, 0.08),
        );
        for z in [-0.39, 0.39] {
            add(
                &parts.wheel,
                &materials.wheel,
                Transform::from_xyz(side * 0.55, -0.08, z)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            );
        }
    }

    // The driver sits behind the controls and faces local -Z, which is also
    // the cart's forward direction.
    add(
        &parts.seat,
        &materials.seat,
        Transform::from_xyz(0.0, 0.62, 0.31),
    );
    add(
        &parts.torso,
        &materials.player,
        Transform::from_xyz(0.0, 0.72, 0.04),
    );
    add(
        &parts.hair,
        &materials.hair,
        Transform::from_xyz(0.0, 1.12, 0.015),
    );
    add(
        &parts.head,
        &materials.skin,
        Transform::from_xyz(0.0, 1.11, -0.055),
    );
    add(
        &parts.hat_brim,
        &materials.player,
        Transform::from_xyz(0.0, 1.34, -0.07),
    );
    add(
        &parts.hat_crown,
        &materials.player,
        Transform::from_xyz(0.0, 1.43, 0.0),
    );
    for side in [-1.0, 1.0] {
        add(
            &parts.arm,
            &materials.player,
            Transform::from_xyz(side * 0.25, 0.72, -0.15)
                .with_rotation(Quat::from_rotation_x(-0.72)),
        );
        add(
            &parts.hand,
            &materials.skin,
            Transform::from_xyz(side * 0.25, 0.56, -0.33),
        );
        add(
            &parts.eye,
            &materials.eye,
            Transform::from_xyz(side * 0.085, 1.15, -0.285),
        );
    }
    add(
        &parts.control_bar,
        &materials.control,
        Transform::from_xyz(0.0, 0.53, -0.39),
    );
    add(
        &parts.target,
        &materials.target,
        Transform::from_xyz(0.0, 0.18, 0.68),
    );
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cart_welds_into_one_mesh_per_material() {
        let mut materials = Assets::<StandardMaterial>::default();
        let mut meshes = Assets::<Mesh>::default();
        let mut handle = || materials.add(StandardMaterial::default());
        let cart_materials = CartMaterials {
            player: handle(),
            skin: handle(),
            hair: handle(),
            metal: handle(),
            wheel: handle(),
            seat: handle(),
            eye: handle(),
            control: handle(),
            target: handle(),
        };
        let welded = cart_model(&CartMeshes::new(), &cart_materials).build(&mut meshes);
        assert_eq!(welded.len(), 9);
    }
}
