//! Explosions: the blast when a missile lands, the wreckage it makes of the
//! scenery around it, and the chain of smaller blasts that ripples outwards
//! through the woods.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::scenery::{Scenery, Transient};
use crate::tracks::SimpleRng;

/// Ground radius a missile blast wrecks scenery in.
const MISSILE_BLAST_RADIUS: f32 = 2.0;

/// A wrecked prop goes up with a blast of its own, smaller and after a beat,
/// so the destruction spreads outwards instead of vanishing all at once. The
/// last link only flashes, which is what keeps a blast from clearing the
/// whole forest.
const CHAIN_BLAST_RADII: [f32; 3] = [MISSILE_BLAST_RADIUS, 1.3, 0.0];

const GRAVITY: f32 = 16.0;

/// The seconds over which a piece of wreckage shrinks away at the end of its life.
const DEBRIS_FADE: f32 = 0.3;

/// How long a missile scorches the ground, and how much of that it spends
/// fading. Short enough that a round of shooting does not blacken the arena.
const SCORCH_SECONDS: f32 = 8.0;
const SCORCH_FADE: f32 = 3.0;

/// Every surface a blast's light reaches pays for it on each frame, so the
/// light stays close to the blast and only a few burn at once. Four carts
/// spraying missiles set off a couple of dozen blasts a second, and lighting
/// the whole arena from each of them was the single largest cost of a fight.
const MAX_FLASHES: usize = 3;
const FLASH_RANGE: f32 = 6.0;

/// A blast waiting for its fuse to run out.
#[derive(Component)]
pub(crate) struct Detonation {
    position: Vec3,
    /// How many links down the chain this blast is.
    chain: usize,
    fuse: f32,
}

/// Queue the blast of a missile that just struck something.
pub(crate) fn detonate(commands: &mut Commands, position: Vec3) {
    commands.spawn((
        Detonation {
            position,
            chain: 0,
            fuse: 0.0,
        },
        Transient,
    ));
}

/// Every explosion draws from one set of meshes and materials. Fire and smoke
/// fade by stepping through pre-made materials, so no asset is ever edited
/// while the game runs.
#[derive(Resource)]
pub(crate) struct EffectAssets {
    sphere: Handle<Mesh>,
    cube: Handle<Mesh>,
    disc: Handle<Mesh>,
    fireball: [Handle<StandardMaterial>; 6],
    smoke: [Handle<StandardMaterial>; 6],
    spark: Handle<StandardMaterial>,
    scorch: Handle<StandardMaterial>,
}

impl EffectAssets {
    pub(crate) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let glow = |color: Color| StandardMaterial {
            base_color: color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        };
        // Unlit, because smoke covers a lot of screen and lit transparent
        // fragments also sample the shadow map and every nearby flash.
        let haze = |alpha: f32| StandardMaterial {
            base_color: Color::srgba(0.56, 0.56, 0.57, alpha),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        };
        Self {
            sphere: meshes.add(
                Sphere::new(1.0)
                    .mesh()
                    .ico(2)
                    .expect("two subdivisions are well within the limit"),
            ),
            cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            disc: meshes.add(Cylinder::new(1.0, 0.02)),
            fireball: [
                Color::srgba(1.00, 0.97, 0.75, 0.95),
                Color::srgba(1.00, 0.78, 0.30, 0.90),
                Color::srgba(1.00, 0.48, 0.10, 0.80),
                Color::srgba(0.85, 0.25, 0.05, 0.60),
                Color::srgba(0.45, 0.12, 0.05, 0.38),
                Color::srgba(0.20, 0.08, 0.05, 0.14),
            ]
            .map(|color| materials.add(glow(color))),
            smoke: [0.50, 0.42, 0.33, 0.24, 0.14, 0.05].map(|alpha| materials.add(haze(alpha))),
            spark: materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.80, 0.35),
                emissive: LinearRgba::rgb(8.0, 3.5, 0.6),
                unlit: true,
                ..default()
            }),
            scorch: materials.add(StandardMaterial {
                base_color: Color::srgba(0.10, 0.08, 0.06, 0.6),
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        }
    }
}

/// Randomness for the shape of blasts and the tumbling of wreckage.
#[derive(Resource)]
pub(crate) struct EffectRng(pub(crate) SimpleRng);

#[derive(Component)]
pub(crate) struct Fireball {
    age: f32,
    duration: f32,
    radius: f32,
}

#[derive(Component)]
pub(crate) struct Flash {
    age: f32,
    duration: f32,
    peak: f32,
}

#[derive(Component)]
pub(crate) struct Smoke {
    age: f32,
    duration: f32,
    start_radius: f32,
    end_radius: f32,
    rise: f32,
}

/// A patch of burnt ground that fades away.
#[derive(Component)]
pub(crate) struct Scorch {
    life: f32,
    radius: f32,
}

/// A loose piece flying through the air: a spark, or part of a wrecked prop.
#[derive(Component)]
pub(crate) struct Debris {
    velocity: Vec3,
    /// Axis and rate of the tumble, in radians per second.
    spin: Vec3,
    life: f32,
    /// The scale it was spawned at, which the fade shrinks from.
    scale: Vec3,
    /// The height it comes to rest at.
    rest: f32,
}

/// What a blast reaches: the props standing around it, the pieces they are
/// built from, and the flashes already burning.
#[derive(SystemParam)]
pub(crate) struct Surroundings<'w, 's> {
    scenery: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Scenery,
            &'static Children,
        ),
    >,
    parts: Query<
        'w,
        's,
        (
            &'static Mesh3d,
            &'static MeshMaterial3d<StandardMaterial>,
            &'static GlobalTransform,
        ),
    >,
    flashes: Query<'w, 's, (), With<Flash>>,
}

/// Run down every fuse and set off the blasts whose time has come.
pub(crate) fn run_detonations(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EffectAssets>,
    mut rng: ResMut<EffectRng>,
    mut detonations: Query<(Entity, &mut Detonation)>,
    world: Surroundings,
) {
    // Two blasts in one frame can reach the same prop; it only comes apart once.
    let mut wrecked = HashSet::new();
    let mut flash_budget = MAX_FLASHES.saturating_sub(world.flashes.iter().count());

    for (entity, mut detonation) in &mut detonations {
        detonation.fuse -= time.delta_secs();
        if detonation.fuse > 0.0 {
            continue;
        }
        commands.entity(entity).despawn();

        let radius = CHAIN_BLAST_RADII[detonation.chain];
        burst(
            &mut commands,
            &assets,
            &mut rng.0,
            detonation.position,
            detonation.chain,
            &mut flash_budget,
        );
        if radius <= 0.0 {
            continue;
        }

        for (prop, transform, prop_scenery, children) in &world.scenery {
            let distance = transform
                .translation
                .xz()
                .distance(detonation.position.xz());
            if distance > radius + prop_scenery.hit_radius || !wrecked.insert(prop) {
                continue;
            }
            wreck(
                &mut commands,
                &world.parts,
                children,
                detonation.position,
                &mut rng.0,
            );
            commands.entity(prop).despawn();
            if detonation.chain + 1 < CHAIN_BLAST_RADII.len() {
                commands.spawn((
                    Detonation {
                        position: transform.translation + Vec3::Y * (prop_scenery.height * 0.35),
                        chain: detonation.chain + 1,
                        fuse: 0.06 + distance * 0.05,
                    },
                    Transient,
                ));
            }
        }
    }
}

/// Send every piece of a prop flying away from the blast.
fn wreck(
    commands: &mut Commands,
    parts: &Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform)>,
    children: &Children,
    blast: Vec3,
    rng: &mut SimpleRng,
) {
    for child in children.iter() {
        let Ok((mesh, material, global)) = parts.get(child) else {
            continue;
        };
        let transform = global.compute_transform();
        let outward = (transform.translation - blast)
            .with_y(0.0)
            .normalize_or(random_direction(rng).with_y(0.0).normalize_or(Vec3::X));
        let velocity = outward * rng.range(3.5, 7.0)
            + Vec3::Y * rng.range(3.0, 7.5)
            + random_direction(rng) * 1.2;
        commands.spawn((
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(material.0.clone()),
            transform,
            Debris {
                velocity,
                spin: random_direction(rng) * rng.range(3.0, 9.0),
                life: rng.range(1.2, 1.9),
                scale: transform.scale,
                rest: transform.scale.max_element() * 0.25,
            },
            Transient,
        ));
    }
}

/// Fire, light, sparks, smoke, and for a missile's own blast a scorched patch
/// of ground. A missile's own blast (`chain` zero) is full size; the links
/// after it are smaller and leave no scorch. The light is spent from
/// `flash_budget`, and skipped once it runs out.
fn burst(
    commands: &mut Commands,
    assets: &EffectAssets,
    rng: &mut SimpleRng,
    position: Vec3,
    chain: usize,
    flash_budget: &mut usize,
) {
    let size = if chain == 0 { 1.0 } else { 0.55 };
    let scorch = chain == 0;
    let centre = position.with_y(position.y.max(0.45 * size));
    for (offset, radius, delay) in [
        (Vec3::ZERO, 1.6, 0.0),
        (random_direction(rng) * 0.5 * size, 1.0, 0.04),
        (random_direction(rng) * 0.6 * size, 0.8, 0.09),
    ] {
        commands.spawn((
            Fireball {
                age: -delay,
                duration: rng.range(0.55, 0.7) * size.sqrt(),
                radius: radius * size,
            },
            Mesh3d(assets.sphere.clone()),
            MeshMaterial3d(assets.fireball[0].clone()),
            Transform::from_translation(centre + offset).with_scale(Vec3::splat(0.01)),
            NotShadowCaster,
            Transient,
        ));
    }

    if *flash_budget > 0 {
        *flash_budget -= 1;
        commands.spawn((
            Flash {
                age: 0.0,
                duration: 0.22,
                peak: 2.5e6 * size,
            },
            PointLight {
                color: Color::srgb(1.0, 0.72, 0.35),
                intensity: 0.0,
                range: FLASH_RANGE * size,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(centre + Vec3::Y * 0.6),
            Transient,
        ));
    }

    let sparks = (14.0 * size) as usize;
    for _ in 0..sparks {
        let direction = random_direction(rng);
        let spread = rng.range(0.03, 0.09) * size.sqrt();
        commands.spawn((
            Mesh3d(assets.cube.clone()),
            MeshMaterial3d(assets.spark.clone()),
            Transform::from_translation(centre).with_scale(Vec3::splat(spread)),
            Debris {
                velocity: direction.with_y(direction.y.abs() + 0.4) * rng.range(5.0, 11.0) * size,
                spin: random_direction(rng) * 12.0,
                life: rng.range(0.5, 0.9),
                scale: Vec3::splat(spread),
                rest: spread * 0.5,
            },
            NotShadowCaster,
            Transient,
        ));
    }

    for _ in 0..4 {
        let offset = random_direction(rng) * 0.45 * size;
        puff(
            commands,
            assets,
            centre + offset.with_y(offset.y.abs()),
            0.25 * size,
            0.95 * size,
            rng.range(1.4, 1.9),
            0.9,
        );
    }

    if scorch {
        let radius = 0.9 * size;
        commands.spawn((
            Scorch {
                life: SCORCH_SECONDS,
                radius,
            },
            Mesh3d(assets.disc.clone()),
            MeshMaterial3d(assets.scorch.clone()),
            Transform::from_xyz(position.x, 0.012, position.z)
                .with_scale(Vec3::new(radius, 1.0, radius)),
            NotShadowCaster,
            Transient,
        ));
    }
}

/// One ball of smoke that grows, rises, and thins out.
pub(crate) fn puff(
    commands: &mut Commands,
    assets: &EffectAssets,
    position: Vec3,
    start_radius: f32,
    end_radius: f32,
    duration: f32,
    rise: f32,
) {
    commands.spawn((
        Smoke {
            age: 0.0,
            duration,
            start_radius,
            end_radius,
            rise,
        },
        Mesh3d(assets.sphere.clone()),
        MeshMaterial3d(assets.smoke[0].clone()),
        Transform::from_translation(position).with_scale(Vec3::splat(start_radius)),
        NotShadowCaster,
        Transient,
    ));
}

pub(crate) fn animate_fireballs(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EffectAssets>,
    mut fireballs: Query<(
        Entity,
        &mut Fireball,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut fireball, mut transform, mut material) in &mut fireballs {
        fireball.age += time.delta_secs();
        let progress = fireball.age / fireball.duration;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        if progress < 0.0 {
            continue;
        }
        transform.scale = Vec3::splat(fireball.radius * (0.25 + 0.75 * ease_out(progress)));
        material.0 = assets.fireball[stage(progress, assets.fireball.len())].clone();
    }
}

pub(crate) fn animate_smoke(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EffectAssets>,
    mut puffs: Query<(
        Entity,
        &mut Smoke,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut smoke, mut transform, mut material) in &mut puffs {
        smoke.age += time.delta_secs();
        let progress = smoke.age / smoke.duration;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        // Smoke swells steadily, so the fireball is not smothered the moment it appears.
        let radius = smoke.start_radius + (smoke.end_radius - smoke.start_radius) * progress;
        transform.scale = Vec3::splat(radius);
        transform.translation.y += smoke.rise * time.delta_secs();
        material.0 = assets.smoke[stage(progress, assets.smoke.len())].clone();
    }
}

pub(crate) fn animate_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut Flash, &mut PointLight)>,
) {
    for (entity, mut flash, mut light) in &mut flashes {
        flash.age += time.delta_secs();
        let progress = flash.age / flash.duration;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        light.intensity = flash.peak * (1.0 - progress).powi(2);
    }
}

pub(crate) fn fade_scorches(
    mut commands: Commands,
    time: Res<Time>,
    mut scorches: Query<(Entity, &mut Scorch, &mut Transform)>,
) {
    for (entity, mut scorch, mut transform) in &mut scorches {
        scorch.life -= time.delta_secs();
        if scorch.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let radius = scorch.radius * (scorch.life / SCORCH_FADE).min(1.0);
        transform.scale = Vec3::new(radius, 1.0, radius);
    }
}

pub(crate) fn move_debris(
    mut commands: Commands,
    time: Res<Time>,
    mut debris: Query<(Entity, &mut Debris, &mut Transform)>,
) {
    let delta = time.delta_secs();
    for (entity, mut piece, mut transform) in &mut debris {
        piece.life -= delta;
        if piece.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        piece.velocity.y -= GRAVITY * delta;
        transform.translation += piece.velocity * delta;
        transform.rotate(Quat::from_scaled_axis(piece.spin * delta));
        if transform.translation.y < piece.rest {
            transform.translation.y = piece.rest;
            piece.velocity = bounce(piece.velocity);
            piece.spin *= 0.5;
        }
        transform.scale = piece.scale * (piece.life / DEBRIS_FADE).min(1.0);
    }
}

/// What is left of a velocity after hitting the ground: most of the fall and
/// some of the slide.
fn bounce(velocity: Vec3) -> Vec3 {
    if velocity.y >= 0.0 {
        return velocity;
    }
    Vec3::new(velocity.x * 0.6, -velocity.y * 0.35, velocity.z * 0.6)
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

/// Which of `count` stages a progress in `[0, 1)` falls in.
fn stage(progress: f32, count: usize) -> usize {
    ((progress.clamp(0.0, 1.0) * count as f32) as usize).min(count - 1)
}

fn random_direction(rng: &mut SimpleRng) -> Vec3 {
    let direction = Vec3::new(
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
        rng.range(-1.0, 1.0),
    );
    direction.normalize_or(Vec3::Y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bounce_reverses_and_damps_the_fall_but_not_a_rise() {
        assert_eq!(bounce(Vec3::new(2.0, -4.0, 1.0)), Vec3::new(1.2, 1.4, 0.6));
        assert_eq!(bounce(Vec3::new(2.0, 3.0, 1.0)), Vec3::new(2.0, 3.0, 1.0));
    }

    #[test]
    fn stages_cover_the_whole_animation_without_running_past_the_last() {
        assert_eq!(stage(0.0, 6), 0);
        assert_eq!(stage(0.5, 6), 3);
        assert_eq!(stage(0.999, 6), 5);
        assert_eq!(stage(1.0, 6), 5);
        assert_eq!(stage(7.0, 6), 5);
    }

    #[test]
    fn the_chain_always_ends() {
        assert_eq!(CHAIN_BLAST_RADII[CHAIN_BLAST_RADII.len() - 1], 0.0);
        assert!(CHAIN_BLAST_RADII.windows(2).all(|pair| pair[1] < pair[0]));
    }

    #[test]
    fn random_directions_are_unit_length() {
        let mut rng = SimpleRng::new(11);
        for _ in 0..100 {
            assert!((random_direction(&mut rng).length() - 1.0).abs() < 1e-4);
        }
    }
}
