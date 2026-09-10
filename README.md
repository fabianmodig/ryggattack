# Ryggattack

Ryggattack is a code-first, local multiplayer arena game built with Rust and
[Bevy](https://bevy.org/). The current proof of concept focuses on the core
idea: get behind another player and land a shot in their back.

## Current proof of concept

- Up to four local players on WASD, the arrow keys, and gamepads
- A join lobby where each device takes a seat, and bots fill the rest
- A new procedurally generated railway map on every launch
- Guaranteed connectivity, no dead ends, and additional random loops
- Automatic movement with junction choices
- Cart collisions reverse both players along their current rails
- Start/exit menu and an in-game Escape pause dialog
- Low-poly 3D arena generated entirely in code
- Rear-hit detection, scoring, 60-second rounds, and restart
- No external assets or visual editor

## Run it

On NixOS or any system with Nix and flakes enabled, run the game directly in
the project environment:

```sh
nix develop 'path:.' --command cargo run
```

The explicit `path:.` also works when the directory has not been initialized as
a Git repository yet.

Alternatively, enter the environment and keep it open for development:

```sh
nix develop 'path:.'
cargo run
```

On another Linux distribution, install the current stable Rust toolchain and
Bevy's Linux system dependencies, then run `cargo run`.

The first build compiles Bevy and can take several minutes. Later builds are
incremental and much faster.

Running `cargo run` outside the Nix environment can fail with an
`XKBNotFound`/`libxkbcommon` error because Bevy loads that Linux library at
runtime.

## Controls

Every seat steers the same way; only the buttons differ.

| Action | WASD seat | Arrow seat | Gamepad seat |
| --- | --- | --- | --- |
| Choose direction at the next junction | W A S D | Arrow keys | D-pad or left stick |
| Fire | Space | Right Ctrl | A / Cross, or right trigger |

| Action | Keyboard |
| --- | --- |
| Pause / resume | Escape |
| Restart after a round | R |

START on the main menu opens the join lobby. Press **up** on a keyboard scheme
or a gamepad to take a seat and **down** to leave it; seats are handed out in
join order. Any seat still empty when the round begins is played by a bot, so
one player against three bots still works. At least one player must join before
the round can start. Enter, the gamepad Start button, or the START ROUND button
begins the round, and Escape goes back to the main menu. The seating is kept
when you restart a round or return to the menu.

If only one of the two keyboard seats is taken, that player can steer with WASD
and the arrow keys interchangeably. Claiming both seats splits them into two
independent players.

The start menu can be controlled with the mouse or with Enter to start and
Escape to exit. During a game, Escape opens a dialog with Resume and Return to
Menu options.

The carts move automatically and can only change direction at intersections.
Hold a direction while approaching a junction to choose that branch. The route
is selected on entry, and the cart follows it smoothly through the junction.
Carts cannot make a 180-degree turn. The white marker shows the front and the red
panel is the vulnerable rear target. A hit only scores when the projectile is
travelling in approximately the same direction as the target—the attacker is
behind them. Scored-on players respawn on their starting rail.

## Source layout

The code is grouped by responsibility. Start with `src/main.rs` to see how the
application is configured and the order in which gameplay systems run.

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Application setup and system scheduling |
| `src/game.rs` | Game states, round timer, start, and restart |
| `src/scene.rs` | Assemble the camera, lighting, arena, tracks, carts, and HUD |
| `src/players/mod.rs` | Player state, device and bot input, movement, and collisions |
| `src/players/roster.rs` | Which device holds each of the four seats |
| `src/players/visuals.rs` | Cart and rider models |
| `src/combat.rs` | Projectiles, rear hits, and scoring |
| `src/ui/mod.rs` | Main menu, pause dialog, and score display |
| `src/ui/lobby.rs` | The join lobby and its seat cards |
| `src/tracks/map.rs` | Generate the connected rail network and grid coordinates |
| `src/tracks/path.rs` | Junction routes, smooth movement, and reversing carts |
| `src/tracks/render.rs` | Rail, track-bed, and sleeper meshes |

`src/tracks/mod.rs` exposes the track functions and types used by the rest of
the game. Tuning constants live with the feature they control. Tests live
alongside the code they check; run them with `cargo test` inside the development
environment. Use `cargo fmt` to format changes.

## Near-term roadmap

1. Improve movement, hit feedback, bot behavior, and round presentation.
2. Add original character and arena art.
3. Add a WebAssembly build and automated deployment.
4. Validate Android and iOS packaging early.
