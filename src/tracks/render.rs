//! Rail, track-bed, and sleeper meshes for every route through the network.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;

use super::map::{RailMap, rail_position};
use super::path::{
    TRACK_CORNER_RADIUS, junction_connections, junction_directions, track_corner_pose,
    track_straight_endpoints,
};

const RAIL_HALF_GAUGE: f32 = 0.26;

const RAIL_WIDTH: f32 = 0.08;

const TRACK_CORNER_STEPS: usize = 24;

pub(crate) fn spawn_tracks(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rail_map: &RailMap,
) {
    let unit_cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let bed_material = materials.add(Color::srgb(0.24, 0.20, 0.17));
    let sleeper_material = materials.add(Color::srgb(0.35, 0.23, 0.13));
    let rail_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.53, 0.58),
        metallic: 0.8,
        perceptual_roughness: 0.3,
        ..default()
    });

    for &(start, end) in &rail_map.edges {
        spawn_track_segment(
            commands,
            &unit_cube,
            &bed_material,
            &sleeper_material,
            &rail_material,
            track_straight_endpoints((start, end)),
            true,
        );
    }

    for node in 0..rail_map.neighbors.len() {
        let mut straight_sleepers_spawned = false;
        for (from, to) in junction_connections(rail_map, node) {
            let directions = junction_directions(from, node, to);
            if directions[0].dot(directions[1]) < -0.99 {
                spawn_track_segment(
                    commands,
                    &unit_cube,
                    &bed_material,
                    &sleeper_material,
                    &rail_material,
                    directions
                        .map(|direction| rail_position(node) + direction * TRACK_CORNER_RADIUS),
                    !straight_sleepers_spawned,
                );
                straight_sleepers_spawned = true;
                continue;
            }

            for (radius, width, elevation, material) in [
                (TRACK_CORNER_RADIUS, 0.82, 0.02, &bed_material),
                (
                    TRACK_CORNER_RADIUS - RAIL_HALF_GAUGE,
                    RAIL_WIDTH,
                    0.16,
                    &rail_material,
                ),
                (
                    TRACK_CORNER_RADIUS + RAIL_HALF_GAUGE,
                    RAIL_WIDTH,
                    0.16,
                    &rail_material,
                ),
            ] {
                commands.spawn((
                    Mesh3d(meshes.add(track_corner_mesh(directions, radius, width))),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(rail_position(node).with_y(elevation)),
                ));
            }
            // Multiway junctions share straight sleepers underneath their
            // switches, rather than overlapping a fan of curved sleepers.
            if rail_map.neighbors[node].len() == 2 {
                for step in 0..=4 {
                    let (position, tangent) =
                        track_corner_pose(node, directions, step as f32 / 4.0);
                    commands.spawn((
                        Mesh3d(unit_cube.clone()),
                        MeshMaterial3d(sleeper_material.clone()),
                        Transform::from_translation(position.with_y(0.10))
                            .looking_to(tangent, Vec3::Y)
                            .with_scale(Vec3::new(1.05, 0.08, 0.11)),
                    ));
                }
            }
        }
    }
}

fn track_corner_mesh(directions: [Vec3; 2], radius: f32, width: f32) -> Mesh {
    let center = (directions[0] + directions[1]) * TRACK_CORNER_RADIUS;
    let mut positions = Vec::<[f32; 3]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut quad = |vertices: [Vec3; 4], face_normals: [Vec3; 4]| {
        let outward = (vertices[1] - vertices[0]).cross(vertices[2] - vertices[0]);
        let indices = if outward.dot(face_normals[0]) > 0.0 {
            [0, 1, 2, 0, 2, 3]
        } else {
            [0, 2, 1, 0, 3, 2]
        };
        for index in indices {
            positions.push(vertices[index].to_array());
            normals.push(face_normals[index].to_array());
        }
    };
    for step in 0..TRACK_CORNER_STEPS {
        let [a, b] = [step, step + 1].map(|index| {
            let angle = index as f32 / TRACK_CORNER_STEPS as f32 * std::f32::consts::FRAC_PI_2;
            -directions[0] * angle.cos() - directions[1] * angle.sin()
        });
        let ring = |radial: Vec3| {
            let inner = center + radial * (radius - width * 0.5);
            let outer = center + radial * (radius + width * 0.5);
            [
                inner.with_y(-0.05),
                outer.with_y(-0.05),
                outer.with_y(0.05),
                inner.with_y(0.05),
            ]
        };
        let [a0, a1, a2, a3] = ring(a);
        let [b0, b1, b2, b3] = ring(b);
        quad([a3, a2, b2, b3], [Vec3::Y; 4]);
        quad([a0, a1, b1, b0], [Vec3::NEG_Y; 4]);
        quad([a1, a2, b2, b1], [a, a, b, b]);
        quad([a0, a3, b3, b0], [-a, -a, -b, -b]);
        if step == 0 {
            quad([a0, a1, a2, a3], [directions[1]; 4]);
        }
        if step + 1 == TRACK_CORNER_STEPS {
            quad([b0, b1, b2, b3], [directions[0]; 4]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; positions.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
}

fn spawn_track_segment(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    bed_material: &Handle<StandardMaterial>,
    sleeper_material: &Handle<StandardMaterial>,
    rail_material: &Handle<StandardMaterial>,
    endpoints: [Vec3; 2],
    spawn_sleepers: bool,
) {
    let [start, end] = endpoints;
    let horizontal = (end.x - start.x).abs() > (end.z - start.z).abs();
    let midpoint = (start + end) * 0.5;
    let length = start.distance(end);

    let bed_scale = if horizontal {
        Vec3::new(length, 0.10, 0.82)
    } else {
        Vec3::new(0.82, 0.10, length)
    };
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(bed_material.clone()),
        Transform::from_xyz(midpoint.x, 0.02, midpoint.z).with_scale(bed_scale),
    ));

    for side in [-RAIL_HALF_GAUGE, RAIL_HALF_GAUGE] {
        let across = if horizontal { Vec3::Z } else { Vec3::X };
        let midpoint = midpoint + across * side;
        let position = Vec3::new(midpoint.x, 0.16, midpoint.z);
        let scale = if horizontal {
            Vec3::new(length, 0.10, RAIL_WIDTH)
        } else {
            Vec3::new(RAIL_WIDTH, 0.10, length)
        };
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(rail_material.clone()),
            Transform::from_translation(position).with_scale(scale),
        ));
    }

    if !spawn_sleepers {
        return;
    }

    for offset in [-0.38, 0.0, 0.38] {
        let position = start.lerp(end, 0.5 + offset);
        let scale = if horizontal {
            Vec3::new(0.11, 0.08, 1.05)
        } else {
            Vec3::new(1.05, 0.08, 0.11)
        };
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(sleeper_material.clone()),
            Transform::from_xyz(position.x, 0.10, position.z).with_scale(scale),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_join_straight_tracks_tangentially_in_every_orientation() {
        for horizontal in [11, 13] {
            for vertical in [7, 17] {
                let directions = junction_directions(horizontal, 12, vertical);
                for (progress, tangent) in [(0.0, -directions[1]), (1.0, directions[0])] {
                    let direction = if progress == 0.0 {
                        directions[1]
                    } else {
                        directions[0]
                    };
                    let neighbor = [horizontal, vertical]
                        .into_iter()
                        .find(|&node| {
                            (rail_position(node) - rail_position(12)).normalize() == direction
                        })
                        .unwrap();
                    let (position, heading) = track_corner_pose(12, directions, progress);
                    let endpoint = track_straight_endpoints((12, neighbor))[0];
                    assert!(position.distance(endpoint) < 1e-5);
                    assert!(heading.dot(tangent) > 0.99999);

                    for side in [-RAIL_HALF_GAUGE, RAIL_HALF_GAUGE] {
                        let mesh =
                            track_corner_mesh(directions, TRACK_CORNER_RADIUS + side, RAIL_WIDTH);
                        let vertices = mesh
                            .attribute(Mesh::ATTRIBUTE_POSITION)
                            .unwrap()
                            .as_float3()
                            .unwrap();
                        for edge in [-RAIL_WIDTH * 0.5, RAIL_WIDTH * 0.5] {
                            let radial = if progress == 0.0 {
                                -directions[0]
                            } else {
                                -directions[1]
                            };
                            let expected = (endpoint - rail_position(12) + radial * (side + edge))
                                .with_y(0.05);
                            assert!(
                                vertices
                                    .iter()
                                    .any(|&v| Vec3::from(v).distance(expected) < 1e-5)
                            );
                        }
                    }
                }
            }
        }
    }
}
