//! Automatic graphics quality: the game picks how much detail to draw from
//! how fast the browser is actually drawing it, so that the frame rate stays
//! high without the player having to find a settings screen.
//!
//! The detail knobs (resolution, anti-aliasing, shadows, effects, forest) are
//! arranged into a ladder of [`QUALITY_LEVELS`], from the cheapest look up to
//! the game's full look. A [`QualityGovernor`] watches the frame rate one
//! window at a time: it steps down quickly when frames are slow, and steps up
//! again only after the frame rate has been comfortably high for a while. A
//! step up that makes the game slow again is taken back, and the next try
//! waits longer, so it never keeps flickering between two levels.
//!
//! Nothing here changes how the game plays; it only trades away detail.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::image::ImageSampler;
use bevy::light::DirectionalLightShadowMap;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::Msaa;
use bevy::window::{PrimaryWindow, WindowRef};

/// How many of the canvas's pixels the 3D view is drawn at before it is
/// stretched to fill it. Fill rate is what an integrated GPU runs out of
/// first, especially on a high-DPI screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Shadows {
    Off,
    Low,
    High,
}

/// Fire, smoke, sparks, and the light a blast throws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Effects {
    Low,
    Medium,
    High,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ForestDensity {
    Sparse,
    Full,
}

/// What is currently drawn. Set only by the quality governor; the rest of the
/// game reads it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct VideoSettings {
    pub(crate) render_scale: RenderScale,
    pub(crate) anti_aliasing: bool,
    pub(crate) shadows: Shadows,
    pub(crate) effects: Effects,
    pub(crate) forest: ForestDensity,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self::level(TOP_LEVEL)
    }
}

/// The quality ladder, cheapest first. Each step up adds the detail that
/// costs least for what it shows: resolution first (fill rate is what a weak
/// GPU runs out of), then shadows, effects, the full forest, and last the
/// anti-aliasing, which only smooths edges.
const QUALITY_LEVELS: [VideoSettings; 7] = [
    VideoSettings {
        render_scale: RenderScale::Half,
        anti_aliasing: false,
        shadows: Shadows::Off,
        effects: Effects::Low,
        forest: ForestDensity::Sparse,
    },
    VideoSettings {
        render_scale: RenderScale::TwoThirds,
        anti_aliasing: false,
        shadows: Shadows::Off,
        effects: Effects::Low,
        forest: ForestDensity::Sparse,
    },
    VideoSettings {
        render_scale: RenderScale::ThreeQuarters,
        anti_aliasing: false,
        shadows: Shadows::Low,
        effects: Effects::Medium,
        forest: ForestDensity::Sparse,
    },
    VideoSettings {
        render_scale: RenderScale::Full,
        anti_aliasing: false,
        shadows: Shadows::Low,
        effects: Effects::Medium,
        forest: ForestDensity::Full,
    },
    VideoSettings {
        render_scale: RenderScale::Full,
        anti_aliasing: false,
        shadows: Shadows::High,
        effects: Effects::Medium,
        forest: ForestDensity::Full,
    },
    VideoSettings {
        render_scale: RenderScale::Full,
        anti_aliasing: false,
        shadows: Shadows::High,
        effects: Effects::High,
        forest: ForestDensity::Full,
    },
    VideoSettings {
        render_scale: RenderScale::Full,
        anti_aliasing: true,
        shadows: Shadows::High,
        effects: Effects::High,
        forest: ForestDensity::Full,
    },
];

/// The game's full look.
pub(crate) const TOP_LEVEL: usize = QUALITY_LEVELS.len() - 1;

impl VideoSettings {
    pub(crate) fn level(level: usize) -> Self {
        QUALITY_LEVELS[level.min(TOP_LEVEL)]
    }

    fn shadow_map_size(&self) -> Option<usize> {
        match self.shadows {
            Shadows::Off => None,
            Shadows::Low => Some(512),
            Shadows::High => Some(1024),
        }
    }
}

/// Below this, averaged over a window, the game steps the quality down.
const SLOW_FPS: f32 = 50.0;
/// Below this a single window is enough to step down, and by two levels.
const VERY_SLOW_FPS: f32 = 30.0;
/// The frame rate a window has to keep for it to count towards stepping up.
/// Just under 60, so a 60 Hz screen's vsync-capped rate counts.
const SMOOTH_FPS: f32 = 57.0;
/// Seconds of frames averaged into one decision.
const WINDOW_SECONDS: f32 = 1.0;
/// A frame longer than this is a hitch (the tab was hidden, an asset loaded,
/// the forest was regrown), not a sign of steady load, and is left out of the
/// frame-rate average...
const HITCH_SECONDS: f32 = 0.25;
/// ...unless this many come in a row: then the machine is simply that slow.
const HITCHES_IN_A_ROW: u32 = 3;
/// Smooth windows needed before the first try at a higher level.
const FIRST_RAISE_WAIT: u32 = 4;
/// The longest the governor waits between tries at a higher level.
const MAX_RAISE_WAIT: u32 = 120;
/// A slow window this soon after stepping up means the step up failed.
const PROBE_WINDOWS: u32 = 4;

/// Chooses the quality level from the measured frame rate.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct QualityGovernor {
    pub(crate) level: usize,
    /// A fixed level from the address or environment: measurements and
    /// screenshots need a known look, so the governor then never moves.
    pinned: bool,
    frames: u32,
    elapsed: f32,
    /// Frames longer than [`HITCH_SECONDS`] in a row.
    long_frames: u32,
    slow_windows: u32,
    smooth_windows: u32,
    /// Smooth windows needed before the next step up.
    raise_wait: u32,
    /// Windows since the last step up, while that step is still on trial.
    probe: Option<u32>,
    /// Windows to ignore after a change, while it settles in.
    settle: u32,
}

impl QualityGovernor {
    pub(crate) fn new(level: usize, pinned: bool) -> Self {
        Self {
            level: level.min(TOP_LEVEL),
            pinned,
            frames: 0,
            elapsed: 0.0,
            long_frames: 0,
            slow_windows: 0,
            smooth_windows: 0,
            raise_wait: FIRST_RAISE_WAIT,
            probe: None,
            // The first seconds after loading are always slow.
            settle: 3,
        }
    }

    /// Feed one frame's duration. Returns the new level when it changes.
    pub(crate) fn observe(&mut self, dt: f32) -> Option<usize> {
        if self.pinned {
            return None;
        }
        if dt > HITCH_SECONDS {
            self.long_frames += 1;
            if self.long_frames < HITCHES_IN_A_ROW {
                return None;
            }
            // Not a hitch: every frame is this slow. Judge it as a window of
            // its own, at the rate these frames came.
            self.long_frames = 0;
            self.frames = 0;
            self.elapsed = 0.0;
            return self.close_window(1.0 / dt);
        }
        self.long_frames = 0;
        self.frames += 1;
        self.elapsed += dt;
        if self.elapsed < WINDOW_SECONDS {
            return None;
        }
        let fps = self.frames as f32 / self.elapsed;
        self.frames = 0;
        self.elapsed = 0.0;
        self.close_window(fps)
    }

    fn close_window(&mut self, fps: f32) -> Option<usize> {
        if self.settle > 0 {
            self.settle -= 1;
            return None;
        }
        self.judge(fps)
    }

    /// Decide on one window's average frame rate.
    fn judge(&mut self, fps: f32) -> Option<usize> {
        if let Some(windows) = self.probe.as_mut() {
            *windows += 1;
        }
        if fps < SLOW_FPS {
            self.smooth_windows = 0;
            self.slow_windows += 1;
            let failed_probe = self.probe.is_some_and(|windows| windows <= PROBE_WINDOWS);
            if failed_probe {
                // That level is too much for this machine: back down at once
                // and wait twice as long before trying it again.
                self.raise_wait = (self.raise_wait * 2).min(MAX_RAISE_WAIT);
                return self.step_down(1);
            }
            if fps < VERY_SLOW_FPS {
                return self.step_down(2);
            }
            if self.slow_windows >= 2 {
                return self.step_down(1);
            }
            return None;
        }
        self.slow_windows = 0;
        if self.probe.is_some_and(|windows| windows > PROBE_WINDOWS) {
            // The last step up held; the next one may come sooner again.
            self.probe = None;
            self.raise_wait = (self.raise_wait / 2).max(FIRST_RAISE_WAIT);
        }
        if fps >= SMOOTH_FPS {
            self.smooth_windows += 1;
            if self.smooth_windows >= self.raise_wait && self.level < TOP_LEVEL {
                self.smooth_windows = 0;
                self.probe = Some(0);
                return self.change_to(self.level + 1);
            }
        } else {
            self.smooth_windows = 0;
        }
        None
    }

    fn step_down(&mut self, levels: usize) -> Option<usize> {
        self.slow_windows = 0;
        self.probe = None;
        self.change_to(self.level.saturating_sub(levels))
    }

    fn change_to(&mut self, level: usize) -> Option<usize> {
        if level == self.level {
            return None;
        }
        self.level = level;
        self.settle = 1;
        Some(level)
    }
}

/// A level fixed from outside: `?quality=N` in the web build's address, or
/// `RYGGATTACK_QUALITY=N` on the desktop. `N` is `0` (cheapest) to
/// [`TOP_LEVEL`] (full look).
fn pinned_level() -> Option<usize> {
    storage::pinned()
        .and_then(|text| text.trim().parse::<usize>().ok())
        .map(|level| level.min(TOP_LEVEL))
}

/// The browser remembers the last level between visits in `localStorage`,
/// so a slow machine starts where it left off instead of at the top. The
/// page's `window` object is reached through `js-sys` alone, which the game
/// already links, so no extra browser bindings are compiled in for it.
#[cfg(target_arch = "wasm32")]
mod storage {
    use js_sys::{Function, Reflect};
    use wasm_bindgen::JsValue;

    const KEY: &str = "ryggattack.quality";

    fn global(name: &str) -> Option<JsValue> {
        Reflect::get(&js_sys::global(), &JsValue::from_str(name))
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null())
    }

    fn method(target: &JsValue, name: &str) -> Option<Function> {
        Reflect::get(target, &JsValue::from_str(name))
            .ok()
            .map(Function::from)
    }

    pub(super) fn read() -> Option<String> {
        let storage = global("localStorage")?;
        method(&storage, "getItem")?
            .call1(&storage, &JsValue::from_str(KEY))
            .ok()?
            .as_string()
    }

    pub(super) fn write(text: &str) {
        // A private window can refuse storage; the level then lasts until
        // the tab closes, which is all that is lost.
        if let Some(storage) = global("localStorage")
            && let Some(set) = method(&storage, "setItem")
        {
            let _ = set.call2(&storage, &JsValue::from_str(KEY), &JsValue::from_str(text));
        }
    }

    /// `N` from `?quality=N` in the page's address.
    pub(super) fn pinned() -> Option<String> {
        let location = global("location")?;
        let search = Reflect::get(&location, &JsValue::from_str("search"))
            .ok()?
            .as_string()?;
        super::query_value(&search, "quality").map(str::to_owned)
    }
}

/// On the desktop the level lasts for the session.
#[cfg(not(target_arch = "wasm32"))]
mod storage {
    pub(super) fn read() -> Option<String> {
        None
    }

    pub(super) fn write(_text: &str) {}

    pub(super) fn pinned() -> Option<String> {
        std::env::var("RYGGATTACK_QUALITY").ok()
    }
}

/// The value of `key` in a URL query string such as `?a=1&quality=3`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn query_value<'a>(search: &'a str, key: &str) -> Option<&'a str> {
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value)
}

/// The level to start at: a pinned one, else the one the last visit ended
/// at, else the full look (a fast machine should never have to climb).
fn starting_governor() -> QualityGovernor {
    if let Some(level) = pinned_level() {
        return QualityGovernor::new(level, true);
    }
    let saved = storage::read().and_then(|text| text.trim().parse::<usize>().ok());
    QualityGovernor::new(saved.unwrap_or(TOP_LEVEL), false)
}

/// The camera that draws the world into [`SceneImage`].
#[derive(Component)]
pub(crate) struct WorldCamera;

/// The full-window picture the world camera's image is shown in.
#[derive(Component)]
struct SceneView;

/// The frame-rate readout, toggled with F3.
#[derive(Component)]
struct FpsText;

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
        let governor = starting_governor();
        let settings = VideoSettings::level(governor.level);
        app.insert_resource(settings)
            .insert_resource(governor)
            .insert_resource(DirectionalLightShadowMap {
                size: settings.shadow_map_size().unwrap_or(512),
            })
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, spawn_scene_view)
            .add_systems(
                PostUpdate,
                (
                    govern_quality,
                    fit_scene_image,
                    apply_settings.run_if(resource_changed::<VideoSettings>),
                    toggle_fps,
                    update_fps,
                )
                    .chain(),
            );
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
}

/// Measure every frame, and move to the level the governor picks.
fn govern_quality(
    time: Res<Time<Real>>,
    mut governor: ResMut<QualityGovernor>,
    mut settings: ResMut<VideoSettings>,
) {
    if let Some(level) = governor.observe(time.delta_secs()) {
        info!("Graphics quality {level}/{TOP_LEVEL}");
        *settings = VideoSettings::level(level);
        storage::write(&level.to_string());
    }
}

/// The size, in pixels, the world is drawn at for a canvas of `window`
/// physical pixels.
pub(crate) fn scaled_size(window: UVec2, scale: RenderScale) -> UVec2 {
    (window.as_vec2() * scale.factor())
        .round()
        .as_uvec2()
        .max(UVec2::ONE)
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
    mut views: Query<&mut Visibility, With<SceneView>>,
    mut lights: Query<&mut DirectionalLight>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
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
}

/// F3 shows or hides the frame rate and the quality level.
fn toggle_fps(
    keys: Res<ButtonInput<KeyCode>>,
    mut fps: Query<&mut Visibility, (With<FpsText>, Without<SceneView>)>,
) {
    if !keys.just_pressed(KeyCode::F3) {
        return;
    }
    for mut visibility in &mut fps {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
    }
}

fn update_fps(
    governor: Res<QualityGovernor>,
    diagnostics: Res<DiagnosticsStore>,
    mut readouts: Query<(&mut Text, &Visibility), With<FpsText>>,
) {
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
    let mode = if governor.pinned { "FIXED" } else { "AUTO" };
    for (mut text, visibility) in &mut readouts {
        if *visibility == Visibility::Hidden {
            continue;
        }
        text.0 = format!(
            "{fps:.0} FPS  {frame_ms:.1} ms  Q{}/{TOP_LEVEL} {mode}",
            governor.level
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run whole windows at `fps` through the governor; the level after each.
    fn run(governor: &mut QualityGovernor, fps: f32, windows: usize) -> Vec<usize> {
        let frames = (fps * WINDOW_SECONDS).ceil() as usize;
        (0..windows)
            .map(|_| {
                for _ in 0..frames {
                    governor.observe(1.0 / fps);
                }
                governor.level
            })
            .collect()
    }

    /// A governor past its start-up settling.
    fn settled(level: usize) -> QualityGovernor {
        let mut governor = QualityGovernor::new(level, false);
        governor.settle = 0;
        governor
    }

    #[test]
    fn the_default_is_the_full_look() {
        let settings = VideoSettings::default();
        assert_eq!(settings, QUALITY_LEVELS[TOP_LEVEL]);
        assert!(settings.anti_aliasing);
        assert_eq!(settings.render_scale, RenderScale::Full);
        assert_eq!(settings.shadows, Shadows::High);
        assert_eq!(settings.effects, Effects::High);
        assert_eq!(settings.forest, ForestDensity::Full);
    }

    #[test]
    fn every_step_up_the_ladder_only_adds_detail() {
        for pair in QUALITY_LEVELS.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            assert_ne!(lower, higher, "two identical levels");
            assert!(lower.render_scale <= higher.render_scale);
            assert!(lower.anti_aliasing <= higher.anti_aliasing);
            assert!(lower.shadows <= higher.shadows);
            assert!(lower.effects <= higher.effects);
            assert!(lower.forest <= higher.forest);
        }
    }

    #[test]
    fn lower_effects_never_draw_more() {
        let [low, medium, high] =
            [Effects::Low, Effects::Medium, Effects::High].map(Effects::budget);
        for (less, more) in [(low, medium), (medium, high)] {
            assert!(less.trail_interval >= more.trail_interval);
            assert!(less.fireballs <= more.fireballs);
            assert!(less.sparks <= more.sparks);
            assert!(less.smoke_puffs <= more.smoke_puffs);
            assert!(less.max_flashes <= more.max_flashes);
        }
    }

    #[test]
    fn a_smooth_frame_rate_keeps_the_full_look() {
        let mut governor = settled(TOP_LEVEL);
        assert!(
            run(&mut governor, 60.0, 30)
                .iter()
                .all(|&level| level == TOP_LEVEL)
        );
    }

    #[test]
    fn a_slow_frame_rate_steps_down_until_it_is_smooth() {
        let mut governor = settled(TOP_LEVEL);
        // Somewhat slow: one level after two slow windows.
        let levels = run(&mut governor, 45.0, 3);
        assert_eq!(levels, vec![TOP_LEVEL, TOP_LEVEL - 1, TOP_LEVEL - 1]);
        // Very slow: two levels at a time, down to the bottom and no further.
        let levels = run(&mut governor, 10.0, 12);
        assert_eq!(*levels.last().unwrap(), 0);
        assert!(levels.windows(2).all(|pair| pair[1] <= pair[0]));
    }

    #[test]
    fn the_start_up_seconds_do_not_count() {
        let mut governor = QualityGovernor::new(TOP_LEVEL, false);
        // Loading is slow; the first three windows are ignored.
        assert!(
            run(&mut governor, 10.0, 3)
                .iter()
                .all(|&level| level == TOP_LEVEL)
        );
        assert!(run(&mut governor, 10.0, 1)[0] < TOP_LEVEL);
    }

    #[test]
    fn a_hitch_is_not_a_slow_frame_rate() {
        let mut governor = settled(TOP_LEVEL);
        for _ in 0..5 {
            // One long frame (a hidden tab, a load) in otherwise smooth play.
            governor.observe(2.0);
            run(&mut governor, 60.0, 1);
        }
        assert_eq!(governor.level, TOP_LEVEL);
    }

    #[test]
    fn a_machine_too_slow_for_whole_windows_still_steps_down() {
        // Two frames a second (software rendering): every frame is longer
        // than a hitch, but they come in a row, so they count.
        let mut governor = QualityGovernor::new(TOP_LEVEL, false);
        for _ in 0..40 {
            governor.observe(0.5);
        }
        assert_eq!(governor.level, 0);
    }

    #[test]
    fn a_smooth_frame_rate_steps_back_up() {
        let mut governor = settled(0);
        let levels = run(&mut governor, 60.0, 80);
        assert_eq!(*levels.last().unwrap(), TOP_LEVEL);
        assert!(levels.windows(2).all(|pair| pair[1] >= pair[0]));
    }

    #[test]
    fn a_failed_step_up_is_taken_back_and_tried_less_often() {
        // A machine that manages level 3 smoothly but not level 4.
        let mut governor = settled(3);
        let mut tries = Vec::new();
        for window in 0..400 {
            let fps = if governor.level >= 4 { 40.0 } else { 60.0 };
            let before = governor.level;
            run(&mut governor, fps, 1);
            if governor.level > before {
                tries.push(window);
            }
            assert!(governor.level <= 4);
        }
        assert!(tries.len() >= 3, "kept trying: {tries:?}");
        let gaps: Vec<_> = tries.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(
            gaps.windows(2).all(|pair| pair[1] >= pair[0]),
            "tries should grow further apart: {tries:?}"
        );
    }

    #[test]
    fn a_pinned_level_never_moves() {
        let mut governor = QualityGovernor::new(2, true);
        run(&mut governor, 5.0, 20);
        run(&mut governor, 120.0, 200);
        assert_eq!(governor.level, 2);
        assert_eq!(QualityGovernor::new(99, true).level, TOP_LEVEL);
    }

    #[test]
    fn the_level_is_read_from_the_address() {
        assert_eq!(query_value("?quality=3", "quality"), Some("3"));
        assert_eq!(query_value("?a=1&quality=0&b=2", "quality"), Some("0"));
        assert_eq!(query_value("?qualityx=3", "quality"), None);
        assert_eq!(query_value("", "quality"), None);
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
