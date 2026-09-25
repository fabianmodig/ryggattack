//! The video settings screen, reached from the main menu and the pause dialog.
//!
//! It is a menu screen like the others: its rows are menu items, so the mouse,
//! the arrow keys, and a pad all move the one highlight. Accept or Right steps
//! the highlighted row's value forwards, Left steps it back, and Escape or the
//! pad's east button returns to the screen it was opened from.

use bevy::prelude::*;

use super::{MenuAction, MenuFocus, MenuInput, MenuItem, menu_button};
use crate::settings::{Cycle, Preset, VideoSettings};

/// One line of the settings screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingRow {
    Preset,
    RenderScale,
    AntiAliasing,
    Shadows,
    Effects,
    Forest,
    ShowFps,
}

impl SettingRow {
    const ALL: [Self; 7] = [
        Self::Preset,
        Self::RenderScale,
        Self::AntiAliasing,
        Self::Shadows,
        Self::Effects,
        Self::Forest,
        Self::ShowFps,
    ];

    fn title(self) -> &'static str {
        match self {
            Self::Preset => "QUALITY",
            Self::RenderScale => "RESOLUTION",
            Self::AntiAliasing => "ANTI-ALIASING",
            Self::Shadows => "SHADOWS",
            Self::Effects => "EFFECTS",
            Self::Forest => "FOREST",
            Self::ShowFps => "SHOW FPS",
        }
    }

    fn value(self, settings: &VideoSettings) -> &'static str {
        match self {
            Self::Preset => settings.matching_preset().label(),
            Self::RenderScale => settings.render_scale.label(),
            Self::AntiAliasing => settings.anti_aliasing.label(),
            Self::Shadows => settings.shadows.label(),
            Self::Effects => settings.effects.label(),
            Self::Forest => settings.forest.label(),
            Self::ShowFps => settings.show_fps.label(),
        }
    }

    /// Step this row's value by `step`, `1` or `-1`.
    pub(crate) fn cycle(self, settings: &mut VideoSettings, step: i32) {
        match self {
            Self::Preset => {
                let preset = match settings.matching_preset() {
                    // From a hand-made mix, forwards starts at the lowest.
                    Preset::Custom if step > 0 => Preset::Low,
                    Preset::Custom => Preset::High,
                    preset => preset.cycled(step),
                };
                settings.apply_preset(preset);
            }
            Self::RenderScale => settings.render_scale = settings.render_scale.cycled(step),
            Self::AntiAliasing => settings.anti_aliasing = settings.anti_aliasing.cycled(step),
            Self::Shadows => settings.shadows = settings.shadows.cycled(step),
            Self::Effects => settings.effects = settings.effects.cycled(step),
            Self::Forest => settings.forest = settings.forest.cycled(step),
            Self::ShowFps => settings.show_fps = settings.show_fps.cycled(step),
        }
    }
}

/// Where the settings screen returns to when it closes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsUi {
    MainMenu,
    Pause,
}

/// The text showing a row's current value.
#[derive(Component)]
struct SettingValue(SettingRow);

pub(super) fn spawn_settings(commands: &mut Commands, origin: SettingsUi, settings: &VideoSettings) {
    commands
        .spawn((
            origin,
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            // See-through, so that a change shows in the scene behind at once.
            BackgroundColor(Color::srgba(0.01, 0.015, 0.03, 0.55)),
            GlobalZIndex(120),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: px(520),
                        padding: UiRect::all(px(30)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: px(10),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.09, 0.15, 0.94)),
                    BorderColor::all(Color::srgb(0.35, 0.48, 0.68)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("VIDEO SETTINGS"),
                        TextFont {
                            font_size: FontSize::Px(36.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    for (order, row) in SettingRow::ALL.into_iter().enumerate() {
                        panel.spawn(setting_row(row, order, settings));
                    }
                    panel.spawn(menu_button(
                        "BACK",
                        MenuAction::CloseSettings,
                        SettingRow::ALL.len(),
                    ));
                    panel.spawn((
                        Text::new(
                            "Up/Down: choose   |   Left/Right or Enter: change   |   Escape or B: back",
                        ),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.58, 0.65, 0.76)),
                    ));
                });
        });
}

fn setting_row(row: SettingRow, order: usize, settings: &VideoSettings) -> impl Bundle {
    (
        Button,
        MenuAction::Setting(row),
        MenuItem { order },
        Node {
            width: px(440),
            height: px(44),
            padding: UiRect::horizontal(px(16)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.13, 0.18, 0.29)),
        BorderColor::all(Color::srgb(0.33, 0.48, 0.72)),
        children![
            (
                Text::new(row.title()),
                TextFont {
                    font_size: FontSize::Px(21.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ),
            (
                SettingValue(row),
                Text::new(format!("<  {}  >", row.value(settings))),
                TextFont {
                    font_size: FontSize::Px(21.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.78, 0.24)),
            ),
        ],
    )
}

/// Left and Right step the highlighted row. Accept and clicks go through
/// `handle_menu_actions`, like every other button.
pub(crate) fn adjust_focused_setting(
    input: Res<MenuInput>,
    focus: Res<MenuFocus>,
    actions: Query<&MenuAction>,
    mut settings: ResMut<VideoSettings>,
) {
    let step = input.right as i32 - input.left as i32;
    if step == 0 {
        return;
    }
    if let Some(MenuAction::Setting(row)) = focus.0.and_then(|entity| actions.get(entity).ok()) {
        row.cycle(&mut settings, step);
    }
}

pub(crate) fn refresh_setting_values(
    settings: Res<VideoSettings>,
    mut values: Query<(&SettingValue, &mut Text)>,
) {
    for (value, mut text) in &mut values {
        let wanted = format!("<  {}  >", value.0.value(&settings));
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

pub(crate) fn settings_are_open(open: Query<(), With<SettingsUi>>) -> bool {
    !open.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Shadows;

    #[test]
    fn the_quality_row_steps_through_the_presets() {
        let mut settings = VideoSettings::preset(Preset::High);
        SettingRow::Preset.cycle(&mut settings, 1);
        assert_eq!(settings.matching_preset(), Preset::Low);
        SettingRow::Preset.cycle(&mut settings, -1);
        assert_eq!(settings.matching_preset(), Preset::High);
    }

    #[test]
    fn a_hand_made_mix_reads_as_custom_and_steps_to_a_preset() {
        let mut settings = VideoSettings::preset(Preset::High);
        SettingRow::Shadows.cycle(&mut settings, -1);
        assert_eq!(settings.shadows, Shadows::Low);
        assert_eq!(SettingRow::Preset.value(&settings), "CUSTOM");
        SettingRow::Preset.cycle(&mut settings, 1);
        assert_eq!(settings.matching_preset(), Preset::Low);
    }
}
