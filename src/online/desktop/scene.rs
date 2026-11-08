//! Swap scene geometry, never the window, camera or settings render target.
use super::*;

type SceneRoots = (
    Without<ChildOf>,
    Or<(
        With<Mesh3d>,
        With<crate::players::Player>,
        With<crate::scenery::Transient>,
        With<super::super::environment::Environment>,
        With<super::super::Replica>,
        With<crate::explosions::Flash>,
        With<crate::ui::Hud>,
    )>,
);
type DesktopUi = Or<(With<Panel>, With<super::super::Status>)>;

pub(super) fn clear(mut commands: Commands, roots: Query<Entity, SceneRoots>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<crate::explosions::EffectAssets>();
}

pub(super) fn enter(
    mut commands: Commands,
    mut desktop: ResMut<Desktop>,
    mut view: ResMut<super::super::View>,
    mut clear: Option<ResMut<ClearColor>>,
    mut lights: Query<&mut DirectionalLight>,
    mut ambient: Query<&mut AmbientLight>,
) {
    *desktop = Desktop::default();
    *view = default();
    commands.spawn((
        super::super::Status,
        Text::new(""),
        TextFont {
            font_size: 18.0.into(),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: px(12),
            left: px(12),
            ..default()
        },
    ));
    if let Some(clear) = clear.as_mut() {
        clear.0 = Color::srgb(0.06, 0.10, 0.12);
    }
    for mut light in &mut lights {
        light.illuminance = 12000.;
    }
    for mut light in &mut ambient {
        light.brightness = 400.;
    }
}

pub(super) fn exit(
    mut commands: Commands,
    panels: Query<Entity, DesktopUi>,
    mut desktop: ResMut<Desktop>,
    mut view: ResMut<super::super::View>,
    mut lights: Query<&mut DirectionalLight>,
    mut ambient: Query<&mut AmbientLight>,
) {
    for entity in &panels {
        commands.entity(entity).despawn();
    }
    commands.queue(|world: &mut World| {
        world.remove_non_send::<NetworkClient>();
    });
    *desktop = default();
    *view = default();
    for mut light in &mut lights {
        light.illuminance = 9000.;
    }
    for mut light in &mut ambient {
        light.brightness = 300.;
    }
}

pub(super) fn fog(clear: Res<ClearColor>, mut fogs: Query<&mut DistanceFog>) {
    for mut fog in &mut fogs {
        fog.color = clear.0;
    }
}
