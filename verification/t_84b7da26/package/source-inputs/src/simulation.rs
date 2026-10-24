//! The one gameplay schedule shared by the client and headless execution.
use crate::{combat, explosions, game, input, players, powerups, ui};
use bevy::prelude::*;

pub const TICK_HZ: u32 = 64;
pub const TICK_DURATION: std::time::Duration =
    std::time::Duration::from_nanos(1_000_000_000 / TICK_HZ as u64);

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) struct Gameplay;

pub(crate) struct SimulationPlugin;
impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<input::SeatInputs>()
            .insert_resource(Time::<Fixed>::from_duration(TICK_DURATION))
            .add_systems(
                FixedUpdate,
                (
                    input::apply_inputs,
                    players::bot_input,
                    powerups::tick_and_drop.run_if(game::round_is_live),
                    players::move_players,
                    players::handle_player_collisions,
                    powerups::collect.run_if(game::round_is_live),
                    combat::fire_projectiles.run_if(game::round_is_live),
                    combat::move_projectiles.run_if(game::round_is_live),
                    combat::strike_scenery.run_if(game::round_is_live),
                    combat::detect_hits.run_if(game::round_is_live),
                    explosions::run_detonations,
                    game::tick_round,
                )
                    .chain()
                    .in_set(Gameplay)
                    .run_if(in_state(game::AppState::Playing))
                    .run_if(ui::gameplay_is_running),
            );
    }
}

/// Animation only; never moves a cart, resolves hits, or advances a round.
pub(crate) struct PresentationPlugin;
impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                explosions::animate_fireballs,
                explosions::animate_smoke,
                explosions::animate_flashes,
                explosions::move_debris,
                explosions::fade_scorches,
            )
                .chain()
                .run_if(in_state(game::AppState::Playing))
                .run_if(ui::gameplay_is_running),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{AppState, Round},
        match_settings::MatchSettings,
        players::Roster,
        settings::VideoSettings,
        tracks::RailMap,
    };

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(State::new(AppState::Playing))
            .init_resource::<Time>()
            .init_resource::<Round>()
            .init_resource::<Roster>()
            .init_resource::<MatchSettings>()
            .init_resource::<powerups::Powerups>()
            .init_resource::<ui::PauseState>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<VideoSettings>()
            .insert_resource(RailMap::from_seed(42))
            .add_plugins((SimulationPlugin, PresentationPlugin))
            .add_systems(
                Startup,
                |mut commands: Commands,
                 mut meshes: ResMut<Assets<Mesh>>,
                 mut materials: ResMut<Assets<StandardMaterial>>,
                 map: Res<RailMap>| {
                    crate::scene::setup_gameplay(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &map,
                        crate::match_settings::Arena::Forest,
                    );
                },
            );
        app.update();
        app
    }

    #[test]
    fn device_free_seat_survives_round_reset_with_no_local_devices() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = app();
        app.world_mut().resource_mut::<MatchSettings>().bots = 1;
        app.world_mut().resource_mut::<input::SeatInputs>().external[3] = true;
        app.world_mut()
            .run_system_once(game::start_new_game)
            .unwrap();
        let mut active: Vec<_> = app
            .world_mut()
            .query::<&players::Player>()
            .iter(app.world())
            .filter(|p| p.active)
            .map(|p| p.id)
            .collect();
        active.sort();
        assert_eq!(
            active,
            vec![0, 3],
            "external humans count before filling bot seats"
        );
    }

    #[test]
    fn external_intent_is_not_replaced_by_bot_ai() {
        let mut app = app();
        {
            let mut inputs = app.world_mut().resource_mut::<input::SeatInputs>();
            inputs.external[0] = true;
            inputs.intents[0] = input::ControlIntent {
                horizontal: 0.0,
                vertical: 1.0,
                fire: true,
            };
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_micros(15625));
        app.world_mut().run_schedule(FixedUpdate);
        let mut query = app
            .world_mut()
            .query::<(&players::Player, &crate::tracks::RailFollower)>();
        let (player, rail) = query.iter(app.world()).find(|(p, _)| p.id == 0).unwrap();
        assert_eq!(
            rail.requested_turn,
            Vec3::NEG_Z,
            "device-free seats must not be treated as bots"
        );
        assert!(player.wants_to_fire);
    }

    fn frame_partitioned_app(frame: std::time::Duration, frames: usize) -> Vec<(usize, Vec3)> {
        let mut app = app();
        app.add_plugins(bevy::time::TimePlugin)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(frame));
        // Real time's first update establishes the clock and has zero delta.
        app.update();
        for _ in 0..frames {
            app.update();
        }
        let mut result: Vec<_> = app
            .world_mut()
            .query::<(&players::Player, &Transform)>()
            .iter(app.world())
            .map(|(p, t)| (p.id, t.translation))
            .collect();
        result.sort_by_key(|(id, _)| *id);
        result
    }

    #[test]
    fn fixed_gameplay_is_identical_at_four_and_sixty_four_render_frames_per_second() {
        let fast = frame_partitioned_app(crate::headless::TICK_DURATION, 64);
        let slow = frame_partitioned_app(std::time::Duration::from_millis(250), 4);
        assert_eq!(fast, slow);
        assert!(
            (fast[0].1.x - -1.9).abs() < 0.0001,
            "one second moves 4.2 rail units"
        );
    }

    #[test]
    fn local_pause_gates_the_shared_fixed_pipeline_before_movement_and_drops() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = app();
        app.insert_resource(ui::MenuInput {
            cancel: true,
            ..default()
        });
        app.world_mut()
            .run_system_once(ui::toggle_pause_dialog)
            .unwrap();
        let before: Vec<_> = app
            .world_mut()
            .query::<(&players::Player, &Transform)>()
            .iter(app.world())
            .map(|(p, t)| (p.id, t.translation))
            .collect();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(crate::headless::TICK_DURATION);
        for _ in 0..64 {
            app.world_mut().run_schedule(FixedUpdate);
        }
        let after: Vec<_> = app
            .world_mut()
            .query::<(&players::Player, &Transform)>()
            .iter(app.world())
            .map(|(p, t)| (p.id, t.translation))
            .collect();
        assert_eq!(before, after);
        assert_eq!(
            app.world().resource::<powerups::Powerups>().until_drop,
            30.0
        );
        app.world_mut()
            .run_system_once(ui::close_pause_dialog)
            .unwrap();
        app.world_mut().run_schedule(FixedUpdate);
        assert!(app.world().resource::<powerups::Powerups>().until_drop < 30.0);
    }

    #[test]
    fn presentation_updates_cannot_move_authoritative_carts() {
        let mut app = app();
        let before: Vec<_> = app
            .world_mut()
            .query::<(&players::Player, &Transform)>()
            .iter(app.world())
            .map(|(p, t)| (p.id, t.translation))
            .collect();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(1));
        app.update();
        let after: Vec<_> = app
            .world_mut()
            .query::<(&players::Player, &Transform)>()
            .iter(app.world())
            .map(|(p, t)| (p.id, t.translation))
            .collect();
        assert_eq!(before, after, "render frames must not advance gameplay");
    }
}
