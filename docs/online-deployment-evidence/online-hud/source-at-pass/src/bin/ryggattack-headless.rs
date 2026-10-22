//! Bounded, no-display executable for exercising the shared offline rules.
use ryggattack::{
    headless::{Simulation, SimulationConfig, Snapshot, TICK_HZ},
    input::ControlIntent,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = SimulationConfig::default();
    let mut ticks = 2048_u64;
    let mut fire = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => ticks = args.next().ok_or("missing --ticks value")?.parse()?,
            "--seed" => config.seed = args.next().ok_or("missing --seed value")?.parse()?,
            "--bots" => config.bots = args.next().ok_or("missing --bots value")?.parse()?,
            "--fire" => fire = true,
            _ => {
                return Err(format!(
                    "unknown argument {arg}; use --ticks N --seed N --bots 0..3 [--fire]"
                )
                .into());
            }
        }
    }
    let mut simulation = Simulation::new(config)?;
    simulation.submit(
        0,
        ControlIntent {
            fire,
            ..Default::default()
        },
    )?;
    print_snapshot(simulation.snapshot());
    for tick in 1..=ticks {
        simulation.step();
        if tick % u64::from(TICK_HZ) == 0 || tick == ticks {
            print_snapshot(simulation.snapshot());
        }
    }
    Ok(())
}

fn print_snapshot(snapshot: Snapshot) {
    let carts = snapshot
        .carts
        .iter()
        .map(|p| {
            format!(
                "{{\"id\":{},\"active\":{},\"alive\":{},\"position\":{:?},\"cooldown\":{}}}",
                p.id, p.active, p.alive, p.position, p.shot_cooldown,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"tick\":{},\"seconds\":{},\"carts\":[{}],\"wins\":{:?},\"projectiles\":{},\"pickups\":{},\"scenery\":{},\"until_drop\":{}}}",
        snapshot.tick,
        snapshot.tick as f64 / f64::from(TICK_HZ),
        carts,
        snapshot.wins,
        snapshot.projectiles,
        snapshot.pickups,
        snapshot.scenery,
        snapshot.until_drop
    );
}
