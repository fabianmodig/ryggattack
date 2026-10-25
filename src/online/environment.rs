//! Cached, cosmetic arena presentation. Only snapshot bodies are destructible.
use super::View;
use crate::match_settings::Arena;
use crate::scenery::{ForestAssets, backdrop_meshes, layout};
use crate::settings::ForestDensity;
use crate::tracks::RailMap;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

type Part = (Handle<Mesh>, Handle<StandardMaterial>, Transform);

#[derive(Component)]
pub(super) struct Environment;

#[derive(Default)]
pub(super) struct Cache {
    edges: Option<Vec<(usize, usize)>>,
    themes: [Vec<Part>; 2],
    applied: Option<(u64, Arena)>,
}

impl Cache {
    fn build(
        &mut self,
        edges: &[(usize, usize)],
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) {
        // A topology-derived seed is not the server's scenery seed. The layout
        // is used ONLY for unreachable backdrop and harmless lava placement.
        // All reachable props come exclusively from authoritative Body records.
        let map = RailMap::from_edges(0, edges.to_vec());
        let props = layout(&map);
        self.themes = [Arena::Forest, Arena::Volcano].map(|arena| {
            let assets = ForestAssets::new(meshes, materials);
            assets.set_arena(arena, materials);
            let mut parts = crate::scene::floor_and_walls(meshes, materials);
            if arena == Arena::Volcano {
                for (index, (_, handle, _)) in parts.iter().enumerate() {
                    materials.get_mut(handle).unwrap().base_color = match index {
                        0 => Color::srgb(0.22, 0.19, 0.23),
                        1 => Color::srgb(0.11, 0.09, 0.13),
                        _ => Color::srgb(0.12, 0.10, 0.14),
                    };
                }
                parts.extend(crate::arena::volcano_parts(meshes, materials, &map, &props));
            }
            // Sparse source meshes are the online baseline, even at low quality.
            // Batching keeps the woodland recognizable without hundreds of draws.
            parts.extend(
                backdrop_meshes(meshes, &assets, &props, arena, ForestDensity::Sparse)
                    .into_iter()
                    .map(|(mesh, material)| (mesh, material, Transform::IDENTITY)),
            );
            parts
        });
        self.edges = Some(edges.to_vec());
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn present(
    mut commands: Commands,
    view: Res<View>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<Entity, With<Environment>>,
    mut clear: ResMut<ClearColor>,
    mut lights: Query<&mut DirectionalLight>,
    mut ambient: Query<&mut AmbientLight>,
    mut cache: Local<Cache>,
) {
    let desired = view
        .identity
        .0
        .zip(view.environment)
        .filter(|_| view.edges.is_some());
    let topology_changed = desired.is_some() && cache.edges != view.edges;
    // Also recover if room-switch cleanup removed our meshless hierarchy root.
    if cache.applied == desired && !topology_changed && (desired.is_none() || !roots.is_empty()) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    cache.applied = desired;
    let Some((_, arena)) = desired else {
        clear.0 = Color::srgb(0.06, 0.10, 0.12);
        return;
    };
    if topology_changed {
        cache.build(view.edges.as_ref().unwrap(), &mut meshes, &mut materials);
    }
    let volcanic = arena == Arena::Volcano;
    clear.0 = if volcanic {
        Color::srgb(0.30, 0.16, 0.19)
    } else {
        Color::srgb(0.62, 0.72, 0.66)
    };
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
    commands
        .spawn((Environment, Transform::IDENTITY, Visibility::default()))
        .with_children(|parent| {
            for (index, (mesh, material, transform)) in
                cache.themes[usize::from(volcanic)].iter().enumerate()
            {
                let mut entity = parent.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    *transform,
                ));
                if index < 2 {
                    entity.insert(NotShadowCaster);
                }
            }
        });
}

#[cfg(test)]
mod tests;
