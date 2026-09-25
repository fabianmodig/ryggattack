//! Video settings: what the player can turn down when the browser cannot keep
//! up, the presets that set several at once, and the systems that apply them.
//!
//! The defaults are the game's full look. Everything here only ever trades
//! away detail; none of it changes how the game plays.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::image::ImageSampler;
use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::window::{PrimaryWindow, WindowRef};

/// Stepping through a setting's values from a menu row.
pub(crate) trait Cycle: Sized + Copy + PartialEq + 'static {
    const ALL: &'static [Self];

    fn label(self) -> &'static str;

    /// The next value along, wrapping at either end. `step` is `1` or `-1`.
    fn cycled(self, step: i32) -> Self {
        let index = Self::ALL
            .iter()
            .position(|value| *value == self)
            .unwrap_or(0) as i32;
        Self::ALL[(index + step).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

/// How many of the canvas's pixels the 3D view is drawn at before it is
/// stretched to fill it. Fill rate is what an integrated GPU runs out of
/// first, especially on a high-DPI screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderScale {
    Half,
    TwoThirds,
    ThreeQuarters,
    Full,
}

impl RenderScale {
    pub(crate) fn factor(self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::TwoThirds => 2.0 / 3.0,
            Self::ThreeQuarters => 0.75,
            Self::Full => 1.0,
        }
    }
}

impl Cycle for RenderScale {
    const ALL: &'static [Self] = &[Self::Half, Self::TwoThirds, Self::ThreeQuarters, Self::Full];

    fn label(self) -> &'static str {
        match self {
            Self::Half => "50%",
            Self::TwoThirds => "67%",
            Self::ThreeQuarters => "75%",
            Self::Full => "100%",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shadows {
    Off,
    Low,
    High,
}

impl Cycle for Shadows {
    const ALL: &'static [Self] = &[Self::Off, Self::Low, Self::High];

    fn label(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Low => "LOW",
            Self::High => "HIGH",
        }
    }
}

/// Fire, smoke, sparks, and the light a blast throws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effects {
    Low,
    Medium,
    High,
}

impl Cycle for Effects {
    const ALL: &'static [Self] = &[Self::Low, Self::Medium, Self::High];

    fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

/// How much of a blast is drawn. `High` is the game's original look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectBudget {
    /// Seconds between the smoke puffs a flying missile leaves.
    pub(crate) trail_interval: f32,
    /// Fire balls per blast, from the largest down.
    pub(crate) fireballs: usize,
    /// Sparks from a missile's own blast; chained blasts throw fewer.
    pub(crate) sparks: f32,
    pub(crate) smoke_puffs: usize,
    /// Blast lights burning at once. Each one is paid for by every lit
    /// surface it reaches.
    pub(crate) max_flashes: usize,
}

impl Effects {
    pub(crate) fn budget(self) -> EffectBudget {
        match self {
            Self::High => EffectBudget {
                trail_interval: 0.03,
                fireballs: 3,
                sparks: 14.0,
                smoke_puffs: 4,
                max_flashes: 3,
            },
            Self::Medium => EffectBudget {
                trail_interval: 0.05,
                fireballs: 2,
                sparks: 8.0,
                smoke_puffs: 2,
                max_flashes: 2,
            },
            Self::Low => EffectBudget {
                trail_interval: 0.1,
                fireballs: 1,
                sparks: 4.0,
                smoke_puffs: 1,
                max_flashes: 0,
            },
        }
    }
}

/// How thick the woods beyond the walls grow. Nothing inside the arena, or
/// near enough to its walls to be blown up, is ever thinned out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ForestDensity {
    Sparse,
    Full,
}

impl Cycle for ForestDensity {
    const ALL: &'static [Self] = &[Self::Sparse, Self::Full];

    fn label(self) -> &'static str {
        match self {
            Self::Sparse => "SPARSE",
            Self::Full => "FULL",
        }
    }
}

impl Cycle for bool {
    const ALL: &'static [Self] = &[false, true];

    fn label(self) -> &'static str {
        if self { "ON" } else { "OFF" }
    }
}

/// A named bundle of settings, or `Custom` once any of them is changed alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Preset {
    Low,
    Medium,
    High,
    Custom,
}

impl Cycle for Preset {
    /// `Custom` is where a preset ends up, not something to pick.
    const ALL: &'static [Self] = &[Self::Low, Self::Medium, Self::High];

    fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
            Self::Custom => "CUSTOM",
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct VideoSettings {
    pub(crate) render_scale: RenderScale,
    pub(crate) anti_aliasing: bool,
    pub(crate) shadows: Shadows,
    pub(crate) effects: Effects,
    pub(crate) forest: ForestDensity,
    pub(crate) show_fps: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self::preset(Preset::High)
    }
}

impl VideoSettings {
    pub(crate) fn preset(preset: Preset) -> Self {
        match preset {
            Preset::High | Preset::Custom => Self {
                render_scale: RenderScale::Full,
                anti_aliasing: true,
                shadows: Shadows::High,
                effects: Effects::High,
                forest: ForestDensity::Full,
                show_fps: false,
            },
            Preset::Medium => Self {
                render_scale: RenderScale::Full,
                anti_aliasing: false,
                shadows: Shadows::Low,
                effects: Effects::Medium,
                forest: ForestDensity::Full,
                show_fps: false,
            },
            Preset::Low => Self {
                render_scale: RenderScale::TwoThirds,
                anti_aliasing: false,
                shadows: Shadows::Off,
                effects: Effects::Low,
                forest: ForestDensity::Sparse,
                show_fps: false,
            },
        }
    }

    /// Which preset these settings match. The FPS counter is not a quality
    /// setting, so it never makes a preset custom.
    pub(crate) fn matching_preset(&self) -> Preset {
        [Preset::Low, Preset::Medium, Preset::High]
            .into_iter()
            .find(|&preset| {
                Self {
                    show_fps: self.show_fps,
                    ..Self::preset(preset)
                } == *self
            })
            .unwrap_or(Preset::Custom)
    }

    /// Switch to a preset, keeping the FPS counter as it was.
    pub(crate) fn apply_preset(&mut self, preset: Preset) {
        *self = Self {
            show_fps: self.show_fps,
            ..Self::preset(preset)
        };
    }

    fn shadow_map_size(&self) -> Option<usize> {
        match self.shadows {
            Shadows::Off => None,
            Shadows::Low => Some(512),
            Shadows::High => Some(1024),
        }
    }

    /// A compact, human-readable form for saving between visits.
    fn encode(&self) -> String {
        format!(
            "scale={} aa={} shadows={} effects={} forest={} fps={}",
            self.render_scale.label(),
            self.anti_aliasing.label(),
            self.shadows.label(),
            self.effects.label(),
            self.forest.label(),
            self.show_fps.label(),
        )
    }

    /// Read what `encode` wrote. Anything unknown or missing keeps its
    /// default, so settings from an older build still load.
    fn decode(text: &str) -> Self {
        fn parse<T: Cycle>(value: &str) -> Option<T> {
            T::ALL.iter().copied().find(|item| item.label() == value)
        }
        let mut settings = Self::default();
        for pair in text.split_whitespace() {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            match key {
                "scale" => settings.render_scale = parse(value).unwrap_or(settings.render_scale),
                "aa" => settings.anti_aliasing = parse(value).unwrap_or(settings.anti_aliasing),
                "shadows" => settings.shadows = parse(value).unwrap_or(settings.shadows),
                "effects" => settings.effects = parse(value).unwrap_or(settings.effects),
                "forest" => settings.forest = parse(value).unwrap_or(settings.forest),
                "fps" => settings.show_fps = parse(value).unwrap_or(settings.show_fps),
                _ => {}
            }
        }
        settings
    }

    /// The settings saved by an earlier visit, or the defaults.
    pub(crate) fn load() -> Self {
        storage::read().map_or_else(Self::default, |text| Self::decode(&text))
    }

    fn save(&self) {
        storage::write(&self.encode());
    }
}

/// The browser keeps settings in `localStorage`. The page's `window` object
/// is reached through `js-sys` alone, which the game already links, so no
/// extra browser bindings are compiled in for it.
#[cfg(target_arch = "wasm32")]
mod storage {
    use js_sys::{Function, Reflect};
    use wasm_bindgen::JsValue;

    const KEY: &str = "ryggattack.video";

    fn local_storage() -> Option<JsValue> {
        Reflect::get(&js_sys::global(), &JsValue::from_str("localStorage"))
            .ok()
            .filter(|storage| !storage.is_undefined() && !storage.is_null())
    }

    fn method(storage: &JsValue, name: &str) -> Option<Function> {
        Reflect::get(storage, &JsValue::from_str(name))
            .ok()
            .map(Function::from)
    }

    pub(super) fn read() -> Option<String> {
        let storage = local_storage()?;
        method(&storage, "getItem")?
            .call1(&storage, &JsValue::from_str(KEY))
            .ok()?
            .as_string()
    }

    pub(super) fn write(text: &str) {
        // A private window can refuse storage; the settings then last until
        // the tab closes, which is all that is lost.
        if let Some(storage) = local_storage()
            && let Some(set) = method(&storage, "setItem")
        {
            let _ = set.call2(&storage, &JsValue::from_str(KEY), &JsValue::from_str(text));
        }
    }
}

/// On the desktop the settings last for the session.
#[cfg(not(target_arch = "wasm32"))]
mod storage {
    pub(super) fn read() -> Option<String> {
        None
    }

    pub(super) fn write(_text: &str) {}
}

/// The camera that draws the world into [`SceneImage`].
#[derive(Component)]
pub(crate) struct WorldCamera;

/// The full-window picture the world camera's image is shown in.
#[derive(Component)]
struct SceneView;

#[derive(Component)]
struct FpsText;

/// The note that suggests a lower quality when the frame rate stays low.
#[derive(Component)]
struct LowFpsHint;

/// Below this frame rate, held for [`LOW_FPS_SECONDS`], the game suggests a
/// lower preset. It suggests once per visit and never changes anything itself.
const LOW_FPS: f64 = 45.0;
const LOW_FPS_SECONDS: f32 = 10.0;
/// How long the suggestion stays on screen.
const HINT_SECONDS: f32 = 8.0;

/// Tracks how long the frame rate has stayed low.
#[derive(Resource, Default)]
struct LowFpsWatch {
    low_for: f32,
    /// Counts down while the note is showing; `None` until it has shown.
    showing: Option<f32>,
}

impl LowFpsWatch {
    /// Feed one frame's smoothed FPS. Returns `true` on the frame the note
    /// should appear.
    fn observe(&mut self, fps: f64, dt: f32, preset: Preset) -> bool {
        if self.showing.is_some() || preset == Preset::Low {
            return false;
        }
        if fps < LOW_FPS {
            self.low_for += dt;
        } else {
            self.low_for = 0.0;
        }
        if self.low_for >= LOW_FPS_SECONDS {
            self.showing = Some(HINT_SECONDS);
            return true;
        }
        false
    }
}

/// The off-screen picture the world is drawn into, at the chosen fraction of
/// the canvas's resolution, and stretched over the window by the UI.
#[derive(Resource)]
pub(crate) struct SceneImage(pub(crate) Handle<Image>);

impl SceneImage {
    pub(crate) fn new(images: &mut Assets<Image>) -> Self {
        let mut image = Image::new_target_texture(
            1,
            1,
            // The canvas's own format on WebGL2 and most desktops.
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
        // Only the GPU ever writes it. The asset stays in the main world too,
        // so that it can be resized when the canvas is.
        image.data = None;
        image.asset_usage = RenderAssetUsages::default();
        image.texture_descriptor.usage |= TextureUsages::TEXTURE_BINDING;
        // The world is redrawn into it every frame, so a resize has nothing
        // worth keeping. Left on (as `new_target_texture` sets it), the resize
        // copies the old texture on the GPU, which it lacks COPY_SRC for; that
        // validation error stops Bevy rendering for good.
        image.copy_on_resize = false;
        // Smooth rather than blocky when a lowered resolution is stretched.
        image.sampler = ImageSampler::linear();
        Self(images.add(image))
    }

    fn target(&self) -> RenderTarget {
        RenderTarget::Image(self.0.clone().into())
    }
}

/// Draws the UI, and, while the world is drawn at a lowered resolution, the
/// stretched picture of it. Idle at full resolution, where the world camera
/// draws straight into the canvas and costs no extra pass.
#[derive(Component)]
struct CanvasCamera;

pub(crate) struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = VideoSettings::load();
        app.insert_resource(settings)
            .insert_resource(DirectionalLightShadowMap {
                size: settings.shadow_map_size().unwrap_or(512),
            })
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, spawn_scene_view)
            .add_systems(
                PostUpdate,
                (
                    fit_scene_image,
                    apply_settings.run_if(resource_changed::<VideoSettings>),
                    update_fps,
                    suggest_lower_quality,
                ),
            )
            .init_resource::<LowFpsWatch>();
    }
}

fn spawn_scene_view(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let scene = SceneImage::new(&mut images);
    commands.spawn((
        CanvasCamera,
        Camera2d,
        Camera {
            order: 1,
            is_active: false,
            ..default()
        },
        Msaa::Off,
    ));
    commands.spawn((
        SceneView,
        ImageNode::new(scene.0.clone()),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(i32::MIN),
        Visibility::Hidden,
    ));
    commands.insert_resource(scene);
    commands.spawn((
        FpsText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.6)),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            right: px(12),
            ..default()
        },
        GlobalZIndex(1000),
        Visibility::Hidden,
    ));
    commands.spawn((
        LowFpsHint,
        Text::new("Low frame rate: try a lower QUALITY under SETTINGS"),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.78, 0.24)),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(12),
            right: px(12),
            ..default()
        },
        GlobalZIndex(1000),
        Visibility::Hidden,
    ));
}

/// Suggest a lower preset once, when the frame rate has stayed under
/// [`LOW_FPS`] for a while. The player decides; nothing changes on its own.
fn suggest_lower_quality(
    time: Res<Time>,
    settings: Res<VideoSettings>,
    diagnostics: Res<DiagnosticsStore>,
    mut watch: ResMut<LowFpsWatch>,
    mut hint: Single<&mut Visibility, With<LowFpsHint>>,
) {
    let dt = time.delta_secs();
    if let Some(left) = watch.showing.as_mut()
        && *left > 0.0
    {
        *left -= dt;
        if *left <= 0.0 {
            **hint = Visibility::Hidden;
        }
        return;
    }
    // The first seconds after loading are always slow; don't count them.
    if time.elapsed_secs() < 5.0 {
        return;
    }
    let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    else {
        return;
    };
    if watch.observe(fps, dt, settings.matching_preset()) {
        **hint = Visibility::Inherited;
    }
}

/// The size, in pixels, the world is drawn at for a canvas of `window`
/// physical pixels.
pub(crate) fn scaled_size(window: UVec2, scale: RenderScale) -> UVec2 {
    (window.as_vec2() * scale.factor()).round().as_uvec2().max(UVec2::ONE)
}

/// Keep the off-screen picture at the chosen fraction of the canvas size.
/// At full resolution it is not drawn into, and stays as small as it can be.
fn fit_scene_image(
    settings: Res<VideoSettings>,
    scene: Res<SceneImage>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
) {
    let wanted = if settings.render_scale == RenderScale::Full {
        UVec2::ONE
    } else {
        scaled_size(
            UVec2::new(window.physical_width(), window.physical_height()),
            settings.render_scale,
        )
    };
    let Some(current) = images.get(&scene.0).map(|image| image.size()) else {
        return;
    };
    if current == wanted {
        return;
    }
    if let Some(mut image) = images.get_mut(&scene.0) {
        image.resize(Extent3d {
            width: wanted.x,
            height: wanted.y,
            depth_or_array_layers: 1,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_settings(
    settings: Res<VideoSettings>,
    scene: Res<SceneImage>,
    mut commands: Commands,
    world_cameras: Query<Entity, With<WorldCamera>>,
    mut canvas_cameras: Query<(Entity, &mut Camera), (With<CanvasCamera>, Without<WorldCamera>)>,
    mut views: Query<&mut Visibility, (With<SceneView>, Without<FpsText>)>,
    mut lights: Query<&mut DirectionalLight>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut fps: Query<&mut Visibility, (With<FpsText>, Without<SceneView>)>,
) {
    let scaled = settings.render_scale != RenderScale::Full;
    for camera in &world_cameras {
        let mut world = commands.entity(camera);
        world.insert(if settings.anti_aliasing {
            Msaa::Sample4
        } else {
            Msaa::Off
        });
        // Whichever camera draws into the canvas also draws the UI.
        if scaled {
            world.insert(scene.target()).remove::<IsDefaultUiCamera>();
        } else {
            world
                .insert(RenderTarget::Window(WindowRef::Primary))
                .insert(IsDefaultUiCamera);
        }
    }
    for (entity, mut camera) in &mut canvas_cameras {
        camera.is_active = scaled;
        if scaled {
            commands.entity(entity).insert(IsDefaultUiCamera);
        } else {
            commands.entity(entity).remove::<IsDefaultUiCamera>();
        }
    }
    for mut visibility in &mut views {
        *visibility = if scaled {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let size = settings.shadow_map_size();
    for mut light in &mut lights {
        light.shadow_maps_enabled = size.is_some();
    }
    if let Some(size) = size
        && shadow_map.size != size
    {
        shadow_map.size = size;
    }
    for mut visibility in &mut fps {
        *visibility = if settings.show_fps {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    settings.save();
}

fn update_fps(
    settings: Res<VideoSettings>,
    diagnostics: Res<DiagnosticsStore>,
    mut text: Single<&mut Text, With<FpsText>>,
) {
    if !settings.show_fps {
        return;
    }
    let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed())
    else {
        return;
    };
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|time| time.smoothed())
        .unwrap_or(0.0);
    text.0 = format!("{fps:.0} FPS  {frame_ms:.1} ms");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_full_look() {
        let settings = VideoSettings::default();
        assert_eq!(settings.matching_preset(), Preset::High);
        assert_eq!(settings.effects.budget(), Effects::High.budget());
        assert_eq!(settings.render_scale, RenderScale::Full);
    }

    #[test]
    fn every_preset_is_recognised_and_a_lone_change_is_custom() {
        for preset in [Preset::Low, Preset::Medium, Preset::High] {
            let mut settings = VideoSettings::preset(preset);
            assert_eq!(settings.matching_preset(), preset);
            settings.show_fps = true;
            assert_eq!(settings.matching_preset(), preset);
        }
        let mut settings = VideoSettings::preset(Preset::High);
        settings.shadows = Shadows::Low;
        assert_eq!(settings.matching_preset(), Preset::Custom);
    }

    #[test]
    fn applying_a_preset_keeps_the_fps_counter() {
        let mut settings = VideoSettings {
            show_fps: true,
            ..default()
        };
        settings.apply_preset(Preset::Low);
        assert!(settings.show_fps);
        assert_eq!(settings.matching_preset(), Preset::Low);
    }

    #[test]
    fn cycling_wraps_both_ways() {
        assert_eq!(Shadows::High.cycled(1), Shadows::Off);
        assert_eq!(Shadows::Off.cycled(-1), Shadows::High);
        assert_eq!(RenderScale::Half.cycled(1), RenderScale::TwoThirds);
        assert!(false.cycled(1));
        // Custom is not in the list, so cycling from it starts over.
        assert_eq!(Preset::Custom.cycled(1), Preset::Medium);
    }

    #[test]
    fn saved_settings_round_trip_and_tolerate_junk() {
        let settings = VideoSettings {
            render_scale: RenderScale::ThreeQuarters,
            anti_aliasing: false,
            shadows: Shadows::Off,
            effects: Effects::Medium,
            forest: ForestDensity::Sparse,
            show_fps: true,
        };
        assert_eq!(VideoSettings::decode(&settings.encode()), settings);
        assert_eq!(
            VideoSettings::decode("scale=13% bogus effects=LOW"),
            VideoSettings {
                effects: Effects::Low,
                ..default()
            }
        );
    }

    #[test]
    fn lower_effects_never_draw_more() {
        let [low, medium, high] = [Effects::Low, Effects::Medium, Effects::High].map(Effects::budget);
        for (less, more) in [(low, medium), (medium, high)] {
            assert!(less.trail_interval >= more.trail_interval);
            assert!(less.fireballs <= more.fireballs);
            assert!(less.sparks <= more.sparks);
            assert!(less.smoke_puffs <= more.smoke_puffs);
            assert!(less.max_flashes <= more.max_flashes);
        }
    }

    #[test]
    fn a_sustained_low_frame_rate_suggests_once() {
        let mut watch = LowFpsWatch::default();
        // A brief dip does not count.
        assert!(!watch.observe(20.0, 5.0, Preset::High));
        assert!(!watch.observe(60.0, 0.1, Preset::High));
        assert!(!watch.observe(20.0, 5.0, Preset::High));
        assert!(watch.observe(20.0, 5.0, Preset::High));
        // Only once.
        assert!(!watch.observe(20.0, 60.0, Preset::High));
        // Already at the lowest preset: nothing to suggest.
        let mut low = LowFpsWatch::default();
        assert!(!low.observe(10.0, 60.0, Preset::Low));
    }

    #[test]
    fn the_scene_image_can_be_resized_without_a_gpu_copy() {
        // A copy on resize needs COPY_SRC, which a render target lacks here;
        // the failed copy used to stop rendering at any scale below 100 %.
        let mut images = Assets::<Image>::default();
        let scene = SceneImage::new(&mut images);
        let image = images.get(&scene.0).expect("just added");
        assert!(!image.copy_on_resize);
        assert!(image.data.is_none());
    }

    #[test]
    fn the_scene_is_drawn_at_the_chosen_fraction() {
        assert_eq!(
            scaled_size(UVec2::new(1920, 1080), RenderScale::Half),
            UVec2::new(960, 540)
        );
        assert_eq!(
            scaled_size(UVec2::new(1920, 1080), RenderScale::Full),
            UVec2::new(1920, 1080)
        );
        assert_eq!(scaled_size(UVec2::ZERO, RenderScale::Half), UVec2::ONE);
    }
}
