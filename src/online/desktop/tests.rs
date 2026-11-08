use super::*;
#[test]
fn paste_button_and_shortcut_fail_visibly_without_opening_a_transport() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    app.world_mut().resource_mut::<Desktop>().selected = 5;
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    assert!(
        app.world()
            .resource::<Desktop>()
            .notice
            .contains("clipboard")
    );
    assert!(app.world().get_non_send::<NetworkClient>().is_none());
    app.world_mut().resource_mut::<Desktop>().editing = Some(Action::Code);
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::ControlLeft);
        keys.press(KeyCode::KeyV);
    }
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    assert!(!app.world().resource::<crate::ui::MenuInput>().accepted());
    assert!(app.world().get_non_send::<NetworkClient>().is_none());
    assert!(app.world().resource::<Entry>().code.is_empty());
}

#[test]
fn menu_input_gate_requires_neutral_then_accepts_new_gameplay_input() {
    let held = crate::input::ControlIntent {
        horizontal: 1.,
        fire: true,
        ..default()
    };
    let mut desktop = Desktop {
        menu: true,
        ..default()
    };
    assert_eq!(desktop.gameplay_intent(held), default());
    desktop.menu = false;
    assert_eq!(desktop.gameplay_intent(held), default());
    assert_eq!(desktop.gameplay_intent(default()), default());
    assert_eq!(desktop.gameplay_intent(held), held);
}

#[test]
fn mouse_can_switch_fields_and_submit_while_editing() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    let mut click = |action: Action| {
        let entity = app
            .world_mut()
            .query_filtered::<(Entity, &Action), With<Button>>()
            .iter(app.world())
            .find(|(_, a)| **a == action)
            .unwrap()
            .0;
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        app.update();
        if let Some(mut interaction) = app.world_mut().get_mut::<Interaction>(entity) {
            *interaction = Interaction::None;
        }
    };
    click(Action::Name);
    click(Action::Address);
    assert_eq!(
        app.world().resource::<Desktop>().editing,
        Some(Action::Address)
    );
    let entity = app
        .world_mut()
        .query_filtered::<(Entity, &Action), With<Button>>()
        .iter(app.world())
        .find(|(_, a)| **a == Action::Create)
        .unwrap()
        .0;
    *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
    app.update();
    assert!(app.world().get_non_send::<NetworkClient>().is_some());
    assert_eq!(app.world().resource::<Desktop>().editing, None);
}

#[test]
fn back_from_setup_returns_to_main_menu_without_a_connection() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    app.world_mut().resource_mut::<Desktop>().selected = 6;
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::MainMenu
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<Panel>>()
            .iter(app.world())
            .next()
            .is_none()
    );
    assert!(app.world().get_non_send::<NetworkClient>().is_none());
}
#[test]
fn editing_unicode_name_consumes_menu_keys_without_connecting() {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .add_message::<KeyboardInput>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyW,
        logical_key: Key::Character(" Åda".into()),
        text: Some(" Åda".into()),
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.world_mut().resource_mut::<crate::ui::MenuInput>().down = true;
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    assert_eq!(app.world().resource::<Entry>().name, "Player Åda");
    assert_eq!(app.world().resource::<Desktop>().selected, 0);
    assert!(!app.world().resource::<crate::ui::MenuInput>().accepted());
    assert!(app.world().get_non_send::<NetworkClient>().is_none());
}
#[test]
fn create_uses_shared_client_without_spawning_another_app() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<AppState>()
        .init_resource::<crate::ui::MenuInput>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Online);
    app.update();
    app.world_mut().resource_mut::<Desktop>().selected = 3;
    app.world_mut()
        .resource_mut::<crate::ui::MenuInput>()
        .accept = true;
    app.update();
    assert!(
        app.world()
            .get_non_send::<crate::net_client::NetworkClient>()
            .is_some()
    );
}
#[test]
fn online_screen_has_editable_connection_fields_and_room_actions() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<crate::game::AppState>()
        .add_plugins(DesktopOnlinePlugin);
    app.world_mut()
        .resource_mut::<NextState<crate::game::AppState>>()
        .set(crate::game::AppState::Online);
    app.update();
    let labels: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|text| text.0.clone())
        .collect();
    for label in [
        "CREATE ROOM",
        "JOIN ROOM",
        "PASTE CODE / INVITE",
        "BACK TO MAIN MENU",
    ] {
        assert!(labels.iter().any(|text| text == label), "missing {label}");
    }
    assert!(
        labels
            .iter()
            .any(|text| text.contains("wss://ryggattack.modig.online/ws"))
    );
}
