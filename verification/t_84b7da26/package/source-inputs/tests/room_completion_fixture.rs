//! Legal-input characterization for the rendered lifecycle fixture, not a
//! scripted outcome. The server's first room uses seed3; the host selects1 win.
use ryggattack::{
    headless::{Arena, Simulation, SimulationConfig},
    input::ControlIntent,
};
#[test]
fn first_to_one_seed_three_reaches_real_victory_under_sampled_inputs() {
    for delay in [0, 4, 16, 32, 64] {
        let mut sim = Simulation::new(SimulationConfig {
            seed: 3,
            human_seats: [true, true, true, false],
            bots: 1,
            wins_target: 1,
            arena: Arena::Volcano,
        })
        .unwrap();
        let mut won = false;
        for tick in 0..8192 {
            if tick >= delay && tick % 8 == 0 {
                // A performs D then releases; B performs Up then releases.
                // The completion portion holds A Left + Fire; B coasts + Fire.
                let a = if tick < 160 {
                    1.
                } else if tick < 768 {
                    0.
                } else {
                    -1.
                };
                let b = if (160..320).contains(&tick) { 1. } else { 0. };
                sim.submit(
                    0,
                    ControlIntent {
                        horizontal: a,
                        vertical: 0.,
                        fire: tick >= 320,
                    },
                )
                .unwrap();
                sim.submit(
                    1,
                    ControlIntent {
                        horizontal: 0.,
                        vertical: b,
                        fire: tick >= 544,
                    },
                )
                .unwrap();
                sim.submit(
                    2,
                    ControlIntent {
                        horizontal: if tick / 128 % 2 == 0 { 1. } else { -1. },
                        vertical: 0.,
                        fire: true,
                    },
                )
                .unwrap();
            }
            sim.step();
            let snapshot = sim.snapshot();
            if snapshot.winner.is_some() {
                println!(
                    "delay {delay} actual first-to-one victory tick {} wins {:?}",
                    snapshot.tick, snapshot.wins
                );
                won = true;
                break;
            }
        }
        assert!(
            won,
            "fixture did not produce an actual victory: {:?}",
            sim.snapshot()
        );
    }
}
