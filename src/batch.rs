//! Baking many copies of a few small meshes into one.
//!
//! The browser build pays for every draw call on the CPU: WebGL2 has no
//! instancing path in Bevy's renderer that survives the web's uniform-buffer
//! limits for this many objects, and each mesh entity costs extraction,
//! culling, queueing, and a draw every frame, twice over when it casts a
//! shadow. Scenery that never moves is therefore welded into one mesh per
//! material, which looks exactly the same and draws in a handful of calls.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

/// Triangles gathered from placed copies of source meshes.
#[derive(Default)]
pub(crate) struct MeshBatch {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshBatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Append a copy of `mesh` placed by `transform`. Meshes without indices
    /// are treated as plain triangle lists.
    pub(crate) fn add(&mut self, mesh: &Mesh, transform: Transform) {
        let Some(positions) = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
        else {
            return;
        };
        let normals = mesh
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(VertexAttributeValues::as_float3);
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(uvs)) => Some(uvs.as_slice()),
            _ => None,
        };

        let base = self.positions.len() as u32;
        let matrix = transform.to_matrix();
        let scale_recip = transform.scale.recip();
        for (index, position) in positions.iter().enumerate() {
            self.positions
                .push(matrix.transform_point3(Vec3::from_array(*position)).to_array());
            let normal = normals.map_or(Vec3::Y, |normals| Vec3::from_array(normals[index]));
            // Normals take the inverse scale, so that a squashed sphere still
            // shades as a squashed sphere, then the rotation.
            let normal = (transform.rotation * (normal * scale_recip).normalize_or(normal)).to_array();
            self.normals.push(normal);
            self.uvs.push(uvs.map_or([0.0, 0.0], |uvs| uvs[index]));
        }
        match mesh.indices() {
            Some(indices) => self
                .indices
                .extend(indices.iter().map(|index| base + index as u32)),
            None => self
                .indices
                .extend((0..positions.len() as u32).map(|index| base + index)),
        }
    }

    pub(crate) fn build(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// One batch per material, kept in the order materials were first used so
/// that the result is deterministic.
#[derive(Default)]
pub(crate) struct MaterialBatches {
    batches: Vec<(Handle<StandardMaterial>, MeshBatch)>,
}

impl MaterialBatches {
    pub(crate) fn add(
        &mut self,
        material: &Handle<StandardMaterial>,
        mesh: &Mesh,
        transform: Transform,
    ) {
        let batch = match self
            .batches
            .iter_mut()
            .position(|(known, _)| known == material)
        {
            Some(index) => &mut self.batches[index].1,
            None => {
                self.batches.push((material.clone(), MeshBatch::default()));
                &mut self.batches.last_mut().expect("just pushed").1
            }
        };
        batch.add(mesh, transform);
    }

    /// Spawn one entity per material, each carrying `extra`.
    pub(crate) fn spawn(
        self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        extra: impl Bundle + Clone,
    ) {
        for (mesh, material) in self.build(meshes) {
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                extra.clone(),
            ));
        }
    }

    /// One welded mesh per material that was used, ready to be spawned.
    pub(crate) fn build(
        self,
        meshes: &mut Assets<Mesh>,
    ) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
        self.batches
            .into_iter()
            .filter(|(_, batch)| !batch.is_empty())
            .map(|(material, batch)| (meshes.add(batch.build()), material))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(mesh: &Mesh) -> Vec<Vec3> {
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap()
            .iter()
            .map(|p| Vec3::from_array(*p))
            .collect()
    }

    #[test]
    fn copies_are_placed_and_indexed_after_each_other() {
        let cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let vertices = cube.count_vertices();
        let indices = cube.indices().unwrap().len();

        let mut batch = MeshBatch::default();
        batch.add(&cube, Transform::IDENTITY);
        batch.add(
            &cube,
            Transform::from_xyz(10.0, 0.0, 0.0).with_scale(Vec3::new(2.0, 1.0, 1.0)),
        );
        let merged = batch.build();

        assert_eq!(merged.count_vertices(), vertices * 2);
        let merged_indices: Vec<usize> = merged.indices().unwrap().iter().collect();
        assert_eq!(merged_indices.len(), indices * 2);
        assert!(merged_indices[indices..].iter().all(|&i| i >= vertices));
        let second = &positions(&merged)[vertices..];
        assert!(second.iter().all(|p| (p.x.abs() - 10.0).abs() <= 1.0 + 1e-5));
        assert!(second.iter().any(|p| (p.x - 11.0).abs() < 1e-5));
    }

    #[test]
    fn normals_stay_unit_length_under_squash_and_rotation() {
        let sphere = Sphere::new(1.0).mesh().ico(1).unwrap();
        let mut batch = MeshBatch::default();
        batch.add(
            &sphere,
            Transform::from_rotation(Quat::from_rotation_x(0.6))
                .with_scale(Vec3::new(0.3, 1.5, 0.7)),
        );
        let merged = batch.build();
        let normals = merged
            .attribute(Mesh::ATTRIBUTE_NORMAL)
            .and_then(VertexAttributeValues::as_float3)
            .unwrap();
        assert!(
            normals
                .iter()
                .all(|n| (Vec3::from_array(*n).length() - 1.0).abs() < 1e-4)
        );
    }

    #[test]
    fn batches_split_by_material() {
        let cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let mut materials = Assets::<StandardMaterial>::default();
        let a = materials.add(StandardMaterial::default());
        let b = materials.add(StandardMaterial::default());
        let mut batches = MaterialBatches::default();
        batches.add(&a, &cube, Transform::IDENTITY);
        batches.add(&b, &cube, Transform::IDENTITY);
        batches.add(&a, &cube, Transform::IDENTITY);
        assert_eq!(batches.batches.len(), 2);
        assert_eq!(
            batches.batches[0].1.positions.len(),
            cube.count_vertices() * 2
        );
    }
}
