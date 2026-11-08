//! Native widgets derive room state exclusively from the shared client.
use super::*;
use crate::rooms::Phase;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Screen {
    Setup,
    Connecting,
    Lobby(bool),
    Playing,
    InputMenu,
    Leaving,
}
#[derive(Component)]
pub(super) struct Info;

#[allow(clippy::too_many_arguments)]
pub(super) fn refresh(
    mut commands: Commands,
    entry: Res<Entry>,
    mut desktop: ResMut<Desktop>,
    client: Option<NonSend<NetworkClient>>,
    panels: Query<Entity, With<Panel>>,
    mut info: Query<&mut Text, With<Info>>,
    mut labels: Query<(&Action, &mut Text), Without<Info>>,
    mut buttons: Query<(&Choice, &Action, &mut BackgroundColor)>,
) {
    let lobby = client.as_ref().and_then(|c| c.lobby.as_ref());
    let running = lobby.is_some_and(|l| l.phase == Phase::Running);
    let host = client
        .as_ref()
        .is_some_and(|c| lobby.is_some_and(|l| Some(l.host) == c.participant));
    let screen = if desktop.leaving {
        Screen::Leaving
    } else if client.is_none() {
        Screen::Setup
    } else if lobby.is_none() {
        Screen::Connecting
    } else if !running {
        Screen::Lobby(host)
    } else if desktop.menu {
        Screen::InputMenu
    } else {
        Screen::Playing
    };
    let detail = match client.as_ref() {
        None => desktop.notice.clone(),
        Some(c) => {
            let room = lobby
                .map(|l| {
                    let humans = l
                        .humans
                        .iter()
                        .map(|h| {
                            format!(
                                "P{}: {}{} — {}",
                                h.seat + 1,
                                h.name,
                                if h.id == l.host { " (host)" } else { "" },
                                if !h.connected {
                                    "disconnected / bot takeover"
                                } else if h.ready {
                                    "ready"
                                } else {
                                    "not ready"
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!(
                        "Room code: {}\n{:?} • {:?} • First to {} • Bots {}\n{}",
                        l.code,
                        l.phase,
                        l.settings.arena,
                        l.settings.wins_target,
                        l.settings.bots,
                        humans
                    )
                })
                .unwrap_or_default();
            format!("{}\n{}\n{}\n{}", c.status, room, c.notice, desktop.notice)
        }
    };
    let options: Vec<_> = match screen {
        Screen::Setup => vec![
            (Action::Name, ""),
            (Action::Address, ""),
            (Action::Code, ""),
            (Action::Create, "CREATE ROOM"),
            (Action::Join, "JOIN ROOM"),
            (Action::Paste, "PASTE CODE / INVITE"),
            (Action::Back, "BACK TO MAIN MENU"),
        ],
        Screen::Connecting => vec![(Action::Leave, "CANCEL CONNECTION")],
        Screen::Lobby(host) => {
            let mut options = vec![(Action::Ready, "READY / UNREADY")];
            if host {
                options.extend([
                    (Action::Bots, "CHANGE BOTS"),
                    (Action::Target, "CHANGE WIN TARGET"),
                    (Action::Arena, "CHANGE ARENA"),
                ]);
            }
            options.extend([
                (Action::CopyInvite, "COPY INVITE LINK"),
                (Action::CopyCode, "COPY ROOM CODE"),
                (Action::Leave, "LEAVE ROOM"),
            ]);
            options
        }
        Screen::InputMenu => vec![
            (Action::Resume, "RESUME INPUT"),
            (Action::CopyInvite, "COPY INVITE LINK"),
            (Action::CopyCode, "COPY ROOM CODE"),
            (Action::Leave, "LEAVE ROOM"),
        ],
        Screen::Playing => vec![(Action::Open, "MENU / ESC")],
        Screen::Leaving => vec![],
    };
    if desktop.screen != Some(screen) {
        for entity in &panels {
            commands.entity(entity).despawn();
        }
        desktop.screen = Some(screen);
        desktop.selected = 0;
        let playing = screen == Screen::Playing;
        let node = if playing {
            Node {
                position_type: PositionType::Absolute,
                bottom: px(16),
                right: px(16),
                padding: UiRect::all(px(6)),
                ..default()
            }
        } else {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(8),
                padding: UiRect::all(px(16)),
                ..default()
            }
        };
        commands.spawn((Panel, node, BackgroundColor(if playing { Color::NONE } else { Color::srgba(0.025, 0.035, 0.065, 0.97) }), GlobalZIndex(105)))
            .with_children(|parent| {
                if !playing {
                    parent.spawn((Text::new(if screen == Screen::InputMenu { "INPUT MENU — SERVER KEEPS RUNNING" } else { "ONLINE MULTIPLAYER" }),
                        TextFont { font_size: 26.0.into(), ..default() }, TextColor(Color::srgb(0.95, 0.78, 0.24))));
                    parent.spawn((Info, Text::new(&detail), Node { width: percent(95), max_width: px(760), ..default() }, TextFont { font_size: 17.0.into(), ..default() }));
                }
                for (order, (action, label)) in options.iter().enumerate() {
                    parent.spawn((Button, *action, Choice(order), Node {
                        width: if playing { px(180) } else { percent(95) }, max_width: px(680), min_height: px(36),
                        padding: UiRect::all(px(6)), justify_content: JustifyContent::Center,
                        border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(8)), ..default()
                    }, BorderColor::all(Color::srgb(0.33, 0.48, 0.72)), BackgroundColor(Color::srgb(0.13, 0.18, 0.29))))
                    .with_child((*action, Text::new(field_label(*action, &entry, desktop.editing).unwrap_or_else(|| label.to_string())), TextFont { font_size: 19.0.into(), ..default() }));
                }
                if !playing {
                    parent.spawn((Text::new(if screen == Screen::Setup {
                        "Select a field to edit • Enter/Esc: finish • Ctrl+A: replace • Ctrl+V: paste\nUp/Down, Tab or D-pad: choose • Enter/A: select"
                    } else { "Up/Down or D-pad: choose • Enter/A: select\nEscape opens the input menu during play; it does not leave the room." }),
                        TextFont { font_size: 15.0.into(), ..default() }));
                }
            });
    } else {
        for mut text in &mut info {
            text.0.clone_from(&detail);
        }
        for (action, mut text) in &mut labels {
            if let Some(label) = field_label(*action, &entry, desktop.editing) {
                text.0 = label;
            }
        }
        for (choice, action, mut color) in &mut buttons {
            color.0 = if desktop.editing == Some(*action) {
                Color::srgb(0.40, 0.29, 0.12)
            } else if choice.0 == desktop.selected {
                Color::srgb(0.22, 0.34, 0.54)
            } else {
                Color::srgb(0.13, 0.18, 0.29)
            };
        }
    }
}

fn field_label(action: Action, entry: &Entry, editing: Option<Action>) -> Option<String> {
    let (label, value) = match action {
        Action::Name => ("Name", &entry.name),
        Action::Address => ("Backend", &entry.address),
        Action::Code => ("Room code", &entry.code),
        _ => return None,
    };
    Some(format!(
        "{label}: {value}{}",
        if editing == Some(action) { " ▏" } else { "" }
    ))
}
