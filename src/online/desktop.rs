//! Native in-window adapter for the shared online client and presentation.
#[cfg(test)]
mod lifecycle_tests;
mod panel;
mod scene;
mod sharing;
#[cfg(test)]
mod tests;

use super::Entry;
use crate::game::AppState;
use crate::net_client::NetworkClient;
use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::prelude::*;

#[derive(Component)]
struct Panel;
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Name,
    Address,
    Code,
    Create,
    Join,
    Back,
    Ready,
    Bots,
    Target,
    Arena,
    Resume,
    Open,
    Leave,
    CopyInvite,
    CopyCode,
    Paste,
}
#[derive(Component)]
struct Choice(usize);
#[derive(Resource, Default)]
pub(super) struct Desktop {
    editing: Option<Action>,
    selected: usize,
    pub(super) menu: bool,
    leaving: bool,
    notice: String,
    screen: Option<panel::Screen>,
    select_all: bool,
    neutral_required: bool,
}

impl Desktop {
    pub(super) fn gameplay_intent(
        &mut self,
        intent: crate::input::ControlIntent,
    ) -> crate::input::ControlIntent {
        if self.menu {
            self.neutral_required = true;
        } else if intent == default() {
            self.neutral_required = false;
        }
        if self.neutral_required {
            default()
        } else {
            intent
        }
    }
}

pub(crate) struct DesktopOnlinePlugin;
impl Plugin for DesktopOnlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Desktop>()
            .init_resource::<super::View>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<KeyboardInput>()
            .init_resource::<crate::ui::MenuInput>()
            .insert_resource(Entry {
                address: "wss://ryggattack.modig.online/ws".into(),
                name: "Player".into(),
                code: String::new(),
            })
            .add_systems(
                OnEnter(AppState::Online),
                (scene::clear, scene::enter, panel::refresh).chain(),
            )
            .add_systems(
                OnExit(AppState::Online),
                (
                    scene::clear,
                    scene::exit,
                    crate::scene::rebuild_world.run_if(resource_exists::<crate::tracks::RailMap>),
                )
                    .chain(),
            )
            .add_systems(
                PreUpdate,
                controls
                    .after(crate::ui::gather_menu_input)
                    .before(crate::ui::navigate_menu)
                    .run_if(in_state(AppState::Online)),
            )
            .add_systems(
                Update,
                (
                    (
                        super::network_input,
                        super::render_snapshot,
                        super::environment::present,
                        scene::fog,
                        (
                            crate::explosions::animate_smoke,
                            crate::explosions::animate_flashes,
                            crate::explosions::move_debris,
                            crate::explosions::fade_scorches,
                        )
                            .chain()
                            .run_if(resource_exists::<crate::explosions::EffectAssets>),
                    )
                        .chain()
                        .run_if(has_client)
                        .run_if(resource_exists::<Assets<Mesh>>),
                    finish_leave,
                    panel::refresh,
                )
                    .chain()
                    .run_if(in_state(AppState::Online)),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn controls(
    mut commands: Commands,
    mut desktop: ResMut<Desktop>,
    mut entry: ResMut<Entry>,
    mut input: ResMut<crate::ui::MenuInput>,
    mut typing: MessageReader<KeyboardInput>,
    choices: Query<(&Choice, &Action, Ref<Interaction>)>,
    keys: Res<ButtonInput<KeyCode>>,
    mut client: Option<NonSendMut<NetworkClient>>,
    mut next: ResMut<NextState<AppState>>,
    mut clipboard: Option<ResMut<Clipboard>>,
) {
    // Always consume events: opening a field must not replay old local input.
    let events: Vec<_> = typing.read().cloned().collect();
    let cancel = input.cancel;
    let clicked = choices
        .iter()
        .find(|(_, _, interaction)| {
            interaction.is_changed() && **interaction == Interaction::Pressed
        })
        .map(|(_, action, _)| *action);
    if let Some(field) = desktop.editing {
        if keys.any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
        ]) && keys.just_pressed(KeyCode::KeyV)
        {
            let result = sharing::read(clipboard.as_deref_mut()).and_then(|text| {
                sharing::paste(&mut entry, Some(field), desktop.select_all, &text)
            });
            desktop.notice = match result {
                Ok(notice) => {
                    desktop.select_all = false;
                    notice.into()
                }
                Err(error) => error,
            };
            *input = default();
            return;
        }
        let value = match field {
            Action::Name => &mut entry.name,
            Action::Address => &mut entry.address,
            _ => &mut entry.code,
        };
        let modifier = keys.any_pressed([
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
        ]);
        for event in events {
            if event.state != ButtonState::Pressed {
                continue;
            }
            match event.key_code {
                KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Escape => {
                    desktop.editing = None;
                    break;
                }
                KeyCode::KeyA if modifier => desktop.select_all = true,
                KeyCode::Backspace => {
                    if desktop.select_all {
                        value.clear();
                        desktop.select_all = false;
                    } else {
                        value.pop();
                    }
                }
                _ if !modifier => {
                    if let Some(text) = event.text {
                        if desktop.select_all {
                            value.clear();
                            desktop.select_all = false;
                        }
                        let max = match field {
                            Action::Name => 24,
                            Action::Code => 20,
                            _ => 256,
                        };
                        for ch in text.chars().filter(|ch| !ch.is_control()) {
                            if value.chars().count() < max {
                                value.push(ch);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if cancel {
            desktop.editing = None;
        }
        *input = default();
        if clicked.is_none() {
            return;
        }
        desktop.editing = None;
    }
    let running = client.as_ref().is_some_and(|c| {
        c.lobby
            .as_ref()
            .is_some_and(|l| l.phase == crate::rooms::Phase::Running)
    });
    if !running {
        desktop.menu = false;
        desktop.neutral_required = true;
    }
    if desktop.leaving {
        *input = default();
        return;
    }
    if running && (input.cancel || input.start) {
        desktop.menu = !desktop.menu;
        *input = default();
        return;
    }
    let count = choices.iter().count();
    if count == 0 {
        *input = default();
        return;
    }
    if !running || desktop.menu {
        if input.down || keys.just_pressed(KeyCode::Tab) {
            desktop.selected = (desktop.selected + 1) % count;
        }
        if input.up {
            desktop.selected = (desktop.selected + count - 1) % count;
        }
    }
    for (choice, _, interaction) in &choices {
        if interaction.is_changed() && *interaction == Interaction::Hovered {
            desktop.selected = choice.0;
        }
    }
    let action = clicked.or_else(|| {
        (input.accepted() && (!running || desktop.menu))
            .then(|| {
                choices
                    .iter()
                    .find(|(choice, _, _)| choice.0 == desktop.selected)
                    .map(|(_, a, _)| *a)
            })
            .flatten()
    });
    *input = default();
    if let Some(client) = client.as_mut() {
        if matches!(action, Some(Action::CopyInvite | Action::CopyCode)) {
            desktop.notice = if let Some(lobby) = &client.lobby {
                let invite = crate::invitation::Invitation {
                    address: entry.address.clone(),
                    code: lobby.code.clone(),
                };
                let web_client = sharing::web_client(&entry.address);
                match sharing::copy(
                    &invite,
                    action == Some(Action::CopyCode),
                    &web_client,
                    |text| {
                        clipboard
                            .as_deref_mut()
                            .ok_or_else(|| "System clipboard is unavailable.".to_owned())?
                            .set_text(text)
                            .map_err(|_| {
                                "Could not access the system clipboard. Try again.".to_owned()
                            })
                    },
                ) {
                    Ok(notice) => notice.into(),
                    Err(error) => error,
                }
            } else {
                "No active room to share.".into()
            };
            return;
        }
        if action == Some(Action::Leave) {
            desktop.leaving = true;
            desktop.menu = true;
            client.leave();
        } else if matches!(action, Some(Action::Open | Action::Resume)) {
            desktop.menu = action == Some(Action::Open);
        } else if let (Some(lobby), Some(participant)) = (&client.lobby, client.participant) {
            let key = match action {
                Some(Action::Ready) => Some(KeyCode::KeyR),
                Some(Action::Bots) => Some(KeyCode::KeyB),
                Some(Action::Target) => Some(KeyCode::KeyT),
                Some(Action::Arena) => Some(KeyCode::KeyV),
                _ => None,
            };
            if let Some(command) = key.and_then(|key| super::lobby_key(key, lobby, participant))
                && let Err(error) = client.command(command)
            {
                client.notice = error;
            }
        }
        return;
    }
    if action == Some(Action::Back) || cancel {
        next.set(AppState::MainMenu);
        return;
    }
    if action == Some(Action::Paste) {
        desktop.notice = match sharing::read(clipboard.as_deref_mut())
            .and_then(|text| sharing::paste(&mut entry, None, true, &text))
        {
            Ok(notice) => notice.into(),
            Err(error) => error,
        };
        return;
    }
    if matches!(action, Some(Action::Name | Action::Address | Action::Code)) {
        desktop.editing = action;
        desktop.select_all = false;
    }
    if matches!(action, Some(Action::Create | Action::Join)) {
        if !crate::invitation::valid_endpoint(&entry.address) {
            desktop.notice = "Enter a valid ws:// or wss:// backend address.".into();
            return;
        }
        let room = (action == Some(Action::Join)).then_some(entry.code.as_str());
        match NetworkClient::connect_room(&entry.address, &entry.name, room) {
            Ok(client) => {
                desktop.notice.clear();
                commands.queue(move |world: &mut World| {
                    world.insert_non_send(client);
                });
            }
            Err(error) => desktop.notice = error,
        }
    }
}

fn has_client(client: Option<NonSend<NetworkClient>>) -> bool {
    client.is_some()
}

fn finish_leave(
    mut commands: Commands,
    mut desktop: ResMut<Desktop>,
    mut client: Option<NonSendMut<NetworkClient>>,
    mut view: ResMut<super::View>,
) {
    if !desktop.leaving {
        return;
    }
    let Some(client) = client.as_mut() else {
        return;
    };
    // leave() may wait for Welcome during admission/reconnect. The shared input
    // pipeline keeps pumping until that seat can be released, not just dropped.
    client.leave();
    if client.status.starts_with("Canceling connection") {
        return;
    }
    desktop.notice.clone_from(&client.status);
    desktop.leaving = false;
    desktop.menu = false;
    *view = default();
    commands.queue(|world: &mut World| {
        use bevy::ecs::system::RunSystemOnce;
        world
            .run_system_once(scene::clear)
            .expect("clear online scene");
        world.remove_non_send::<NetworkClient>();
    });
}
