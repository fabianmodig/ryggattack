//! The forest around and between the rails: where every tree, bush, and rock
//! stands, how each is built, and how the lot is swept away when a round
//! resets. What happens when one is blown apart lives in `explosions`.

use bevy::prelude::*;

use crate::batch::MaterialBatches;
use crate::scene::ARENA_HALF_SIZE;
use crate::settings::{ForestDensity, VideoSettings};
use crate::tracks::{RailMap, SimpleRng};

/// Anything a round reset sweeps away: props, wreckage, missiles, and effects.
#[derive(Component)]
pub(crate) struct Transient;

/// A prop standing in the world, ready to be blown apart.
#[derive(Component)]
pub(crate) struct Scenery {
    /// Ground-plane radius a missile has to come within to strike the prop.
    pub(crate) hit_radius: f32,
    /// Top of the prop above the ground. Missiles fly over anything lower.
    pub(crate) height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropKind {
    Pine,
    Birch,
    Bush,
    Rock,
    Stump,
    Fern,
}

impl PropKind {
    /// How far the prop keeps from a rail centre line at scale one, so carts
    /// and their riders pass without clipping it.
    fn track_clearance(self) -> f32 {
        match self {
            Self::Pine | Self::Birch => 1.15,
            Self::Bush | Self::Rock => 0.95,
            Self::Stump | Self::Fern => 0.85,
        }
    }

    /// Ground radius two props keep between them, at scale one.
    fn footprint(self) -> f32 {
        match self {
            Self::Pine => 0.7,
            Self::Birch => 0.6,
            Self::Bush => 0.45,
            Self::Rock => 0.4,
            Self::Stump | Self::Fern => 0.3,
        }
    }

    fn hit_radius(self) -> f32 {
        match self {
            Self::Pine => 0.32,
            Self::Birch => 0.28,
            Self::Bush | Self::Rock => 0.4,
            Self::Stump | Self::Fern => 0.2,
        }
    }

    pub(crate) fn height(self) -> f32 {
        match self {
            Self::Pine => 2.1,
            Self::Birch => 1.8,
            Self::Bush => 0.62,
            Self::Rock => 0.55,
            Self::Fern => 0.5,
            Self::Stump => 0.36,
        }
    }

    /// Whether the prop is low enough to leave the view of the arena open
    /// when it stands between the camera and the near wall.
    fn is_low(self) -> bool {
        self.height() < 1.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Prop {
    pub(crate) kind: PropKind,
    /// Where it stands on the ground.
    pub(crate) position: Vec2,
    pub(crate) yaw: f32,
    pub(crate) scale: f32,
    /// Picks between the shades and shapes a kind comes in.
    variant: u8,
}

/// The layout of the forest, fixed for the life of the map, and the meshes
/// and materials every prop is built from.
#[derive(Resource)]
pub(crate) struct Forest {
    pub(crate) props: Vec<Prop>,
    assets: ForestAssets,
}

struct ForestAssets {
    cylinder: Handle<Mesh>,
    cone: Handle<Mesh>,
    frond: Handle<Mesh>,
    sphere: Handle<Mesh>,
    bark: Handle<StandardMaterial>,
    birch_bark: Handle<StandardMaterial>,
    needles: [Handle<StandardMaterial>; 2],
    leaves: [Handle<StandardMaterial>; 2],
    bush: Handle<StandardMaterial>,
    rock: Handle<StandardMaterial>,
    fern: Handle<StandardMaterial>,
    cut_wood: Handle<StandardMaterial>,
}

/// Props scattered between the rails, inside the walls.
const ARENA_PROPS: usize = 64;

/// Props in the woods beyond the walls, which the camera sees up to the far
/// edge of its frame.
const OUTER_PROPS: usize = 440;

/// The strip just outside the near wall sits between the camera and the
/// arena, so only low growth stands there.
const NEAR_STRIP: f32 = ARENA_HALF_SIZE + 0.8;

const OUTER_X: f32 = 34.0;
const OUTER_FAR_Z: f32 = -30.0;
const OUTER_NEAR_Z: f32 = 14.0;

const ARENA_KINDS: [(PropKind, u64); 6] = [
    (PropKind::Pine, 30),
    (PropKind::Birch, 14),
    (PropKind::Bush, 22),
    (PropKind::Rock, 10),
    (PropKind::Stump, 8),
    (PropKind::Fern, 16),
];

const OUTER_KINDS: [(PropKind, u64); 6] = [
    (PropKind::Pine, 56),
    (PropKind::Birch, 18),
    (PropKind::Bush, 12),
    (PropKind::Rock, 5),
    (PropKind::Stump, 3),
    (PropKind::Fern, 6),
];

impl Forest {
    pub(crate) fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        rail_map: &RailMap,
    ) -> Self {
        let rough = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.95,
            ..default()
        };
        Self {
            props: layout(rail_map),
            assets: ForestAssets {
                // Bevy's default of 32 sides is far finer than a trunk a few
                // pixels wide or a pine tier seen from twenty units off can
                // show: the outline moves by well under a pixel, and the
                // forest is most of the scene's triangles.
                cylinder: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(16)),
                cone: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(24)),
                // Fern fronds are a couple of pixels thick.
                frond: meshes.add(Cone::new(1.0, 1.0).mesh().resolution(8)),
                sphere: meshes.add(
                    Sphere::new(1.0)
                        .mesh()
                        .ico(2)
                        .expect("two subdivisions are well within the limit"),
                ),
                bark: materials.add(rough(Color::srgb(0.36, 0.24, 0.14))),
                birch_bark: materials.add(rough(Color::srgb(0.86, 0.86, 0.80))),
                needles: [
                    materials.add(rough(Color::srgb(0.10, 0.32, 0.14))),
                    materials.add(rough(Color::srgb(0.16, 0.42, 0.18))),
                ],
                leaves: [
                    materials.add(rough(Color::srgb(0.32, 0.58, 0.22))),
                    materials.add(rough(Color::srgb(0.48, 0.62, 0.20))),
                ],
                bush: materials.add(rough(Color::srgb(0.20, 0.45, 0.18))),
                rock: materials.add(rough(Color::srgb(0.45, 0.46, 0.44))),
                fern: materials.add(rough(Color::srgb(0.24, 0.52, 0.20))),
                cut_wood: materials.add(rough(Color::srgb(0.76, 0.62, 0.40))),
            },
        }
    }
}

/// Where everything stands. Deterministic for a map, so a restart grows the
/// same forest back.
fn layout(rail_map: &RailMap) -> Vec<Prop> {
    let mut rng = SimpleRng::new(rail_map.seed ^ 0x5eed_f0e5_7000_0001);
    let mut props = Vec::with_capacity(ARENA_PROPS + OUTER_PROPS);

    let inside = ARENA_HALF_SIZE - 0.6;
    scatter(
        &mut rng,
        &mut props,
        ARENA_PROPS,
        &ARENA_KINDS,
        (0.75, 1.15),
        |rng| Vec2::new(rng.range(-inside, inside), rng.range(-inside, inside)),
        |prop| {
            rail_map.distance_to_track(prop.position) >= prop.kind.track_clearance() * prop.scale
        },
    );

    scatter(
        &mut rng,
        &mut props,
        OUTER_PROPS,
        &OUTER_KINDS,
        (1.2, 2.2),
        |rng| {
            Vec2::new(
                rng.range(-OUTER_X, OUTER_X),
                rng.range(OUTER_FAR_Z, OUTER_NEAR_Z),
            )
        },
        |prop| {
            let outside_walls =
                prop.position.x.abs() > NEAR_STRIP || prop.position.y.abs() > NEAR_STRIP;
            outside_walls && (prop.position.y < NEAR_STRIP || prop.kind.is_low())
        },
    );

    props
}

/// Drop `count` props into a region, keeping the ones that pass `accept` and
/// stand clear of everything placed before them.
fn scatter(
    rng: &mut SimpleRng,
    props: &mut Vec<Prop>,
    count: usize,
    kinds: &[(PropKind, u64)],
    scale: (f32, f32),
    region: impl Fn(&mut SimpleRng) -> Vec2,
    accept: impl Fn(&Prop) -> bool,
) {
    let mut placed = 0;
    for _ in 0..count * 12 {
        if placed == count {
            break;
        }
        let prop = Prop {
            kind: weighted_kind(rng, kinds),
            position: region(rng),
            yaw: rng.range(0.0, std::f32::consts::TAU),
            scale: rng.range(scale.0, scale.1),
            variant: rng.index(2) as u8,
        };
        if !accept(&prop) || props.iter().any(|other| overlaps(&prop, other)) {
            continue;
        }
        props.push(prop);
        placed += 1;
    }
}

fn weighted_kind(rng: &mut SimpleRng, kinds: &[(PropKind, u64)]) -> PropKind {
    let total: u64 = kinds.iter().map(|(_, weight)| weight).sum();
    let mut pick = rng.index(total as usize) as u64;
    for &(kind, weight) in kinds {
        if pick < weight {
            return kind;
        }
        pick -= weight;
    }
    kinds[kinds.len() - 1].0
}

fn overlaps(left: &Prop, right: &Prop) -> bool {
    let separation = left.kind.footprint() * left.scale + right.kind.footprint() * right.scale;
    left.position.distance_squared(right.position) < separation * separation
}

/// How far beyond the walls a blast can reach. Missiles never leave the
/// arena; a missile's blast wrecks props a chain-blast radius away plus
/// their own size, and each wrecked prop blasts again one smaller step
/// further out. A test checks this covers the whole chain.
pub(crate) const BLAST_REACH: f32 = 5.5;

/// How far a spot on the ground lies outside the arena's walls.
fn distance_outside_arena(position: Vec2) -> f32 {
    (position.abs() - Vec2::splat(ARENA_HALF_SIZE))
        .max(Vec2::ZERO)
        .length()
}

impl Prop {
    /// Whether any blast could ever reach this prop. The ones that cannot
    /// are the backdrop, which never changes and is drawn as a few big meshes.
    pub(crate) fn can_be_wrecked(&self) -> bool {
        distance_outside_arena(self.position) <= BLAST_REACH
    }
}

/// The part of the forest no blast reaches, welded into one mesh per material.
#[derive(Component, Clone)]
pub(crate) struct Backdrop;

/// Every prop a blast can reach, each its own entity that can come apart.
pub(crate) fn spawn_forest(commands: &mut Commands, forest: &Forest) {
    for prop in forest.props.iter().filter(|prop| prop.can_be_wrecked()) {
        spawn_prop(commands, &forest.assets, prop);
    }
}

/// The far woods. Every prop keeps its exact shape, place, and material; they
/// are only drawn together. `Sparse` leaves out half of them.
pub(crate) fn spawn_backdrop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    forest: &Forest,
    density: ForestDensity,
) {
    // Welded in patches rather than all at once, so that the patches out of
    // view, and out of the shadow's reach, are still culled.
    let mut patches: Vec<(IVec2, MaterialBatches)> = Vec::new();
    let backdrop = forest
        .props
        .iter()
        .filter(|prop| !prop.can_be_wrecked())
        .enumerate()
        .filter(|(index, _)| density == ForestDensity::Full || index % 2 == 0);
    for (_, prop) in backdrop {
        let cell = (prop.position / BACKDROP_PATCH).floor().as_ivec2();
        let batches = match patches.iter().position(|(known, _)| *known == cell) {
            Some(index) => &mut patches[index].1,
            None => {
                patches.push((cell, MaterialBatches::default()));
                &mut patches.last_mut().expect("just pushed").1
            }
        };
        let placement = Transform::from_xyz(prop.position.x, 0.0, prop.position.y)
            .with_rotation(Quat::from_rotation_y(prop.yaw))
            .with_scale(Vec3::splat(prop.scale));
        for (mesh, material, transform) in parts(&forest.assets, prop) {
            let Some(mesh) = meshes.get(mesh) else {
                continue;
            };
            batches.add(material, mesh, placement * transform);
        }
    }
    for (_, batches) in patches {
        batches.spawn(commands, meshes, Backdrop);
    }
}

/// Side of the square patches the backdrop is welded in.
const BACKDROP_PATCH: f32 = 14.0;

/// Grow the backdrop, and grow it again whenever the forest setting changes.
pub(crate) fn apply_forest_density(
    mut commands: Commands,
    settings: Res<VideoSettings>,
    forest: Option<Res<Forest>>,
    mut meshes: ResMut<Assets<Mesh>>,
    backdrop: Query<Entity, With<Backdrop>>,
    mut grown: Local<Option<ForestDensity>>,
) {
    let Some(forest) = forest else {
        return;
    };
    if *grown == Some(settings.forest) {
        return;
    }
    for entity in &backdrop {
        commands.entity(entity).despawn();
    }
    spawn_backdrop(&mut commands, &mut meshes, &forest, settings.forest);
    *grown = Some(settings.forest);
}

/// Sweep away props, wreckage, and effects alike and grow the forest back.
/// The backdrop is never touched, so it stays.
pub(crate) fn rebuild_forest(
    commands: &mut Commands,
    forest: &Forest,
    transient: &Query<Entity, With<Transient>>,
) {
    for entity in transient.iter() {
        commands.entity(entity).despawn();
    }
    spawn_forest(commands, forest);
}

fn spawn_prop(commands: &mut Commands, assets: &ForestAssets, prop: &Prop) {
    commands
        .spawn((
            Scenery {
                hit_radius: prop.kind.hit_radius() * prop.scale,
                height: prop.kind.height() * prop.scale,
            },
            Transient,
            Transform::from_xyz(prop.position.x, 0.0, prop.position.y)
                .with_rotation(Quat::from_rotation_y(prop.yaw))
                .with_scale(Vec3::splat(prop.scale)),
            Visibility::default(),
        ))
        .with_children(|parent| {
            for (mesh, material, transform) in parts(assets, prop) {
                parent.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    transform,
                ));
            }
        });
}

/// The pieces a prop is built from, in its own frame at scale one. Each is a
/// separate entity so that an explosion can send them flying one by one.
fn parts<'a>(
    assets: &'a ForestAssets,
    prop: &Prop,
) -> Vec<(&'a Handle<Mesh>, &'a Handle<StandardMaterial>, Transform)> {
    let variant = usize::from(prop.variant);
    let at = |x: f32, y: f32, z: f32, scale: Vec3| Transform::from_xyz(x, y, z).with_scale(scale);
    match prop.kind {
        PropKind::Pine => {
            let needles = &assets.needles[variant];
            vec![
                (
                    &assets.cylinder,
                    &assets.bark,
                    at(0.0, 0.35, 0.0, Vec3::new(0.09, 0.7, 0.09)),
                ),
                (
                    &assets.cone,
                    needles,
                    at(0.0, 0.72, 0.0, Vec3::new(0.62, 0.85, 0.62)),
                ),
                (
                    &assets.cone,
                    needles,
                    at(0.0, 1.22, 0.0, Vec3::new(0.48, 0.75, 0.48)),
                ),
                (
                    &assets.cone,
                    needles,
                    at(0.0, 1.72, 0.0, Vec3::new(0.32, 0.72, 0.32)),
                ),
            ]
        }
        PropKind::Birch => {
            let leaves = &assets.leaves[variant];
            let lean = if variant == 0 { 0.25 } else { -0.22 };
            vec![
                (
                    &assets.cylinder,
                    &assets.birch_bark,
                    at(0.0, 0.5, 0.0, Vec3::new(0.07, 1.0, 0.07)),
                ),
                (
                    &assets.sphere,
                    leaves,
                    at(0.0, 1.3, 0.0, Vec3::new(0.55, 0.5, 0.55)),
                ),
                (
                    &assets.sphere,
                    leaves,
                    at(lean, 1.5, 0.1, Vec3::new(0.38, 0.35, 0.38)),
                ),
            ]
        }
        PropKind::Bush => vec![
            (
                &assets.sphere,
                &assets.bush,
                at(0.0, 0.24, 0.0, Vec3::new(0.34, 0.28, 0.34)),
            ),
            (
                &assets.sphere,
                &assets.bush,
                at(0.28, 0.2, 0.1, Vec3::new(0.26, 0.22, 0.26)),
            ),
            (
                &assets.sphere,
                &assets.bush,
                at(-0.24, 0.18, -0.14, Vec3::new(0.24, 0.2, 0.24)),
            ),
        ],
        PropKind::Rock => vec![
            (
                &assets.sphere,
                &assets.rock,
                at(0.0, 0.16, 0.0, Vec3::new(0.5, 0.36, 0.42)),
            ),
            (
                &assets.sphere,
                &assets.rock,
                at(0.35, 0.12, 0.15, Vec3::new(0.28, 0.22, 0.26)),
            ),
        ],
        PropKind::Stump => vec![
            (
                &assets.cylinder,
                &assets.bark,
                at(0.0, 0.17, 0.0, Vec3::new(0.18, 0.34, 0.18)),
            ),
            (
                &assets.cylinder,
                &assets.cut_wood,
                at(0.0, 0.35, 0.0, Vec3::new(0.15, 0.03, 0.15)),
            ),
        ],
        PropKind::Fern => [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
            .into_iter()
            .map(|lean| {
                // Each frond leans away from the centre: tilt the cone about
                // the axis perpendicular to the direction it leans in.
                let axis = Vec3::new(lean.y, 0.0, -lean.x);
                let transform = Transform::from_xyz(lean.x * 0.08, 0.22, lean.y * 0.08)
                    .with_rotation(Quat::from_axis_angle(axis, 0.55))
                    .with_scale(Vec3::new(0.07, 0.5, 0.07));
                (&assets.frond, &assets.fern, transform)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forests() -> impl Iterator<Item = (RailMap, Vec<Prop>)> {
        (1..=20).map(|seed| {
            let map = RailMap::from_seed(seed);
            let props = layout(&map);
            (map, props)
        })
    }

    #[test]
    fn props_inside_the_walls_keep_clear_of_the_rails() {
        for (map, props) in forests() {
            let inside: Vec<_> = props
                .iter()
                .filter(|prop| {
                    prop.position.x.abs() < ARENA_HALF_SIZE
                        && prop.position.y.abs() < ARENA_HALF_SIZE
                })
                .collect();
            assert!(inside.len() >= ARENA_PROPS / 2, "seed {}", map.seed);
            for prop in inside {
                assert!(
                    map.distance_to_track(prop.position)
                        >= prop.kind.track_clearance() * prop.scale,
                    "{prop:?} stands on a rail of map {}",
                    map.seed
                );
            }
        }
    }

    #[test]
    fn props_never_overlap() {
        for (_, props) in forests() {
            for (index, left) in props.iter().enumerate() {
                for right in &props[index + 1..] {
                    assert!(!overlaps(left, right), "{left:?} overlaps {right:?}");
                }
            }
        }
    }

    #[test]
    fn only_low_growth_stands_between_the_camera_and_the_near_wall() {
        for (_, props) in forests() {
            for prop in props.iter().filter(|prop| prop.position.y > NEAR_STRIP) {
                assert!(prop.kind.is_low(), "{prop:?} would hide the arena");
            }
        }
    }

    #[test]
    fn the_woods_beyond_the_walls_are_dense() {
        for (_, props) in forests() {
            let outside = props
                .iter()
                .filter(|prop| {
                    prop.position.x.abs() > ARENA_HALF_SIZE
                        || prop.position.y.abs() > ARENA_HALF_SIZE
                })
                .count();
            assert!(outside >= OUTER_PROPS * 9 / 10);
        }
    }

    #[test]
    fn no_blast_reaches_the_backdrop() {
        let widest_prop = [
            PropKind::Pine,
            PropKind::Birch,
            PropKind::Bush,
            PropKind::Rock,
            PropKind::Stump,
            PropKind::Fern,
        ]
        .map(PropKind::hit_radius)
        .into_iter()
        .fold(0.0, f32::max)
            * 2.2;
        // Every link of the chain that wrecks anything carries the blast one
        // radius plus one prop further out.
        let reach: f32 = crate::explosions::CHAIN_BLAST_RADII
            .iter()
            .filter(|radius| **radius > 0.0)
            .map(|radius| radius + widest_prop)
            .sum();
        assert!(reach < BLAST_REACH, "{reach} >= {BLAST_REACH}");
    }

    #[test]
    fn the_backdrop_is_most_of_the_forest_and_none_of_the_arena() {
        for (_, props) in forests() {
            let backdrop: Vec<_> = props.iter().filter(|prop| !prop.can_be_wrecked()).collect();
            assert!(backdrop.len() > props.len() / 2);
            assert!(backdrop.iter().all(|prop| {
                prop.position.x.abs() > ARENA_HALF_SIZE + BLAST_REACH
                    || prop.position.y.abs() > ARENA_HALF_SIZE + BLAST_REACH
                    || distance_outside_arena(prop.position) > BLAST_REACH
            }));
        }
    }

    #[test]
    fn the_same_map_grows_the_same_forest() {
        let map = RailMap::from_seed(3);
        assert_eq!(layout(&map), layout(&map));
        assert_ne!(layout(&map), layout(&RailMap::from_seed(4)));
    }
}
