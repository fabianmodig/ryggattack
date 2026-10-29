# Ryggattack

Ryggattack is a code-first, local multiplayer arena game built with Rust and
[Bevy](https://bevy.org/). The current proof of concept focuses on the core
idea: get behind another player and land a shot in their back.

## Current proof of concept

- Up to four local players on WASD, the arrow keys, and gamepads
- A join lobby with adjustable 0–3 bots (capped by empty seats), a win target, and arena selection
- A new procedurally generated railway map on every launch
- Guaranteed connectivity, no dead ends, and additional random loops
- Automatic movement with junction choices
- Rear-end cart collisions nudge only the front cart forward along its rails; head-on and side impacts still reverse both carts
- Menus driven by mouse, keyboard, or gamepad, and an in-game pause dialog
- Two code-generated arenas: forest and volcano (basalt, lava river/pools, and glowing craters)
- Missiles that explode on impact and blow apart the trees around them
- One-life rear-hit elimination, one win for the last survivor, and automatic next rounds
- Configurable first-to 1, 3, 5, or 7 wins (default 3); no round timer
- Solo zero-bot practice without scoring; simultaneous all-dead rounds are draws
- Exactly 500 ms fire cooldown
- Random rail powerups: +25% speed or all-side elimination, with active-effect HUD timers
- No external assets or visual editor

## Shared simulation / headless foundation

The offline native/WASM client and `ryggattack-headless` now run the same
64 Hz gameplay chain. The windowless harness accepts steering/fire intent and
exercises real rails, bots, collisions, missiles, destructible scenery,
powerups, wins, and round resets without a display:

```sh
cargo run --locked --bin ryggattack-headless -- --seed 42 --ticks 2048 --bots 0 --fire
python3 scripts/headless-smoke.py
```

This is the shared-simulation foundation for issue #6. Rendering dependencies
and CPU-side visual construction remain coupled to some gameplay systems.
See [the foundation notes](docs/online-foundation.md) for the reusable Rust API.
`cargo run` continues to open the ordinary offline game.

## Private online rooms (issue #6, phases 3–5)

The authoritative WebSocket service defaults to loopback and runs isolated rooms, one
human seat per connected device, with up to four humans plus bots total.
Names are anonymous display labels, not identities. Create a room and share
its 20-character unguessable code/link; joining a lobby can replace a bot.

```sh
cargo run --locked --bin ryggattack-server -- 127.0.0.1:8787
# Creates a room; its HUD shows the code:
cargo run --locked --bin ryggattack -- --online ws://127.0.0.1:8787 --name Ada
# Join that room from another native client:
cargo run --locked --bin ryggattack -- --online ws://127.0.0.1:8787 --name Ben --room <code>
```

Build/serve the browser bundle as below, then choose **PLAY LOCAL** or
**PLAY ONLINE** from the regular menu. Online setup has explicit create/join
buttons, with the same-origin backend selected by default and custom addresses
under **Advanced**. A shared `?online=...&room=...` link opens setup with the code
prefilled; it does not join until **JOIN ROOM** is selected, and it does not grant
ownership of a previous player's seat. A valid scoped saved seat resumes on
reload. See [integrated menu lifecycle and verification](docs/unified-menu.md).
Native online controls: **R** ready/unready; host **B** bots, **T** win target,
**V** arena; **L** deliberately leaves. After leaving, **F2** creates a room,
or type a code (Backspace edits) and press **Enter** to join. The launch name
comes from `--name`. Browser controls also expose explicit buttons/settings
and a selectable share link. Both show the host and every human's readiness.

The browser online HUD shows active-seat names (with bot fallbacks), round wins,
IN/OUT and coexisting effect timers, plus named authoritative results. Connection
feedback stays visible; tick/RTT diagnostics are collapsible. Setup and room
lobby occupy the menu; during gameplay only the HUD and contextual input menu
are shown, never the create/join form. Native CLI online entry remains supported. See
[HUD lifecycle, fallback and verification](docs/online-hud.md).

Online cart travel uses bounded shared-rule local prediction/reconciliation and
rail-aware delayed remote interpolation when the server supplies the optional
protocol-4 movement capability. Collisions, damage, pickups and outcomes remain
server-authoritative; older servers fall back to authoritative poses. See
[the movement design and acceptance matrix](docs/client-prediction.md) and
[continuation verification and limitations](docs/client-prediction-continuation.md).
These development notes do not imply that this branch is deployed.

All humans must ready up for a two-second server countdown. Joining, unreadying
or disconnecting cancels it; changing host settings clears readiness. Bots
run the actual shared AI and do not ready up. One human with bots works;
zero-bot solo practice remains offline. New joins are locked during a match,
even if a seat is free. A victory displays for three server seconds, then
returns connected players to the **same lobby**, preserving its settings and
clearing readiness. Disconnects elect the oldest connected human as host and
reserve the seat for 60 wall seconds. During play the actual shared bot AI
controls the same character; private reconnect restores its current state,
without respawning dead players or reclaiming former-host privileges. The
last human leaving freezes simulation; eligible reconnect resumes it, and
60 seconds empty deletes the room. Browser and native adapters retry with
bounded backoff; browser reload uses endpoint/room-scoped sessionStorage,
not the shareable invitation. Deliberate Leave clears the saved claim and
stops retrying. See [resilience policies and verification](docs/online-resilience.md).
Phase 5 adds explicit production binding, same-origin WSS, bounded admission,
private health/metrics, hardened paired images and a single-replica Helm chart.
See [production setup, shutdown/reset and rollback](docs/online-deployment.md).
Local verification is not proof of a public rollout; persistence is not provided.

Without the online opt-in, offline play is unchanged. Online Escape suppresses
your input rather than pausing the server. Replicated visuals remain development
proxies. See [room protocol and limits](docs/online-rooms.md). Public deployments
must opt into the production policy, never simply reverse-proxy development
mode. Development browser origins default to exactly
`http://127.0.0.1:8000` and `http://localhost:8000`. For port 8080, set
`RYGGATTACK_ALLOWED_ORIGINS=http://127.0.0.1:8080,http://localhost:8080` on the
server. Native clients without Origin remain allowed.

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
Bevy's Linux system dependencies, then run `cargo run`. Bevy 0.19 needs Rust
1.95 or newer; older toolchains stop with an `is not supported by the
following packages` error before compiling anything.

The first build compiles Bevy and can take several minutes. Later builds are
incremental and much faster.

Running `cargo run` outside the Nix environment can fail with an
`XKBNotFound`/`libxkbcommon` error because Bevy loads that Linux library at
runtime.

## Run it in a browser

The game also builds to WebAssembly and runs in any browser with WebGL 2.

The web build needs an extra Rust target and the `wasm-bindgen` CLI, so use a
rustup-managed toolchain rather than the Nix shell:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128
```

The CLI version has to match the `wasm-bindgen` crate pinned in `Cargo.lock`.
`scripts/build-web.sh` compares the two and prints the exact `cargo install`
command when they drift apart, so start there if the page loads to a blank
canvas after a dependency bump.

Build the bundle and serve it:

```sh
scripts/build-web.sh
python3 -m http.server 8080 --directory dist/web
```

Then open <http://127.0.0.1:8080/>. The page has to come from a web server;
opening `dist/web/index.html` from disk does not work because browsers refuse
to load ES modules and WebAssembly over `file://`.

`scripts/build-web.sh --debug` builds the unoptimized profile. It compiles
faster after a source change but produces a far larger download and a much
lower frame rate, so it is only worth it while iterating.

### What the build produces

Everything lands in `dist/web/`, which is ignored by Git:

| File | Contents |
| --- | --- |
| `index.html` | Page shell and canvas, copied from `web/index.html` |
| `ryggattack.js` | JavaScript glue generated by `wasm-bindgen` |
| `ryggattack_bg.wasm` | The game itself |
| `assets/` | Copy of the repository's `assets/` directory |

Any static host will do — upload `dist/web/` as it is. Serve `.wasm` with the
`application/wasm` content type and turn on compression: the release bundle is
roughly 39 MB uncompressed and 12 MB gzipped, because it carries all of Bevy.

A release build also runs `wasm-opt -Oz` over the module when Binaryen is
installed, which takes about a quarter off the uncompressed size and close to
nothing off the compressed one. It is optional because it is the one tool here
that cargo does not install; the build says which of the two it produced.

### The playable build

The canonical deployment target is **https://ryggattack.modig.online/**, serving
the game and same-origin `/ws` through the Kubernetes external ingress. Public
readiness must be confirmed by the deployment owner. GitHub Pages publishes
only a redirect to that origin, preserving invitations and replacing obsolete
backend URLs; it no longer hosts an independent gameplay artifact.

The repository lives on `git.modig.online` and is push-mirrored to GitHub.
`.github/workflows/pages.yml` runs only there: Forgejo ignores
`.github/workflows` because `.forgejo/workflows` exists. Pull requests build
`dist/web/` in the `web build` job of `ci.yml` on Forgejo, without publishing
anything.

### The published container image

A main release builds/tests separate web and backend images, publishes them
with the same `sha-<short source commit>` tag (plus convenience `latest`), then
commits the paired chart `appVersion` with `[skip ci]`. The Helm chart never
deploys `latest`; no publication occurs from a manual branch run. Both pushes
and container smoke must succeed before chart advancement:

```sh
docker run --rm -p 8080:8080 git.modig.online/fabianmodig/ryggattack/web:latest
docker run --rm -p 8080:8080 git.modig.online/fabianmodig/ryggattack/web:sha-<short-sha>
```

The image is nginx on port 8080 with `dist/web/` as its document root, the
module and the glue stored pre-compressed, and `.wasm` answered as
`application/wasm`. Nothing is built from tags.

`.forgejo/workflows/release.yml` builds both images and exercises real hardened
HTTP/health/WebSocket containers before pushing either. Pushing needs the
`PACKAGE_USER` and `PACKAGE_TOKEN` Actions secrets. Starting the workflow by
hand from another branch builds and smoke-tests the image without publishing
it. Chart commit-back additionally needs the scoped workflow token to have
repository-write permission. First-publication/bootstrap and the private
management port are documented in [production setup](docs/online-deployment.md).

The web image builds locally, from the repository root:

```sh
scripts/build-web.sh
docker build -f web/Dockerfile -t ryggattack-web .
docker run --rm -p 8080:8080 ryggattack-web
```

`web/Dockerfile` packages a bundle that already exists; it does not compile
one, so `scripts/build-web.sh` has to run first.

### How the web build differs

- The game renders into the `#ryggattack-canvas` element from
  `web/index.html` and follows the size of the window.
- That canvas has to hold keyboard focus. The page focuses it on load and on
  every pointer press; a game embedded in another page needs to do the same.
- The main menu has no EXIT button and Escape does not quit there, because a
  page cannot close itself. Escape still opens the in-game pause dialog.
- Gamepads arrive through the browser's Gamepad API, so a pad only shows up in
  the lobby after a button on it is pressed with the page focused.
- A browser without hardware acceleration falls back to software rendering.
  Bevy logs a warning about that in the developer console and the game runs,
  but slowly.

## Controls

Every seat steers the same way; only the buttons differ.

| Action | WASD seat | Arrow seat | Gamepad seat |
| --- | --- | --- | --- |
| Choose direction at the next junction | W A S D | Arrow keys | D-pad or left stick |
| Fire | Space | Right Ctrl | A / Cross, or right trigger |

| Action | Keyboard | Gamepad |
| --- | --- | --- |
| Move the menu highlight | Arrow keys or WASD | D-pad or left stick |
| Select the highlighted button | Enter or Space | A / Cross, or Start |
| Back / cancel | Escape | B / Circle |
| Pause / resume | Escape | Start to pause, B or A on RESUME to resume |
| Restart after a match | R or click RESTART MATCH | Start |

Every menu can equally be clicked with the mouse, driven with the keyboard, or
driven with a gamepad; the three share one highlight, so hovering a button with
the mouse also moves the keyboard and gamepad selection there.

START on the main menu opens the join lobby. Press **up** on a keyboard scheme
or a gamepad to take a seat and **down** to leave it; seats are handed out in
join order. **BOTS** cycles from zero to the number of empty seats, filling the
lowest free seats first; the remaining seats say EMPTY and do not enter play.
At least one human must join before START MATCH works. One human with zero bots
starts unlimited solo practice: move and fire normally, but never score or
automatically restart.

Up and down are taken by joining and leaving in the lobby. **Left/Right** (or
A/D, gamepad D-pad/stick) moves the highlight through START MATCH, BOTS, FIRST TO,
ARENA, and BACK. Enter, Space, gamepad A/Cross, or Start activates the highlighted
button; mouse clicks do the same. BOTS cycles its count, FIRST TO cycles 3 → 5 →
7 → 1 → 3, and ARENA toggles FOREST/VOLCANO. START MATCH starts out highlighted.
Escape or B goes back. Seating and settings survive returning to the menu.

If only one of the two keyboard seats is taken, that player can steer with WASD
and the arrow keys interchangeably. Claiming both seats splits them into two
independent players.

During a game, Escape or the gamepad Start button opens a dialog with Resume and
Return to Menu options.

The carts move automatically and can only change direction at intersections.
Hold a direction while approaching a junction to choose that branch. The route
is selected on entry, and the cart follows it smoothly through the junction.
Carts cannot make a 180-degree turn. The white marker shows the front and the red
panel is the vulnerable rear target. A missile travelling in approximately the
same direction as its target eliminates it immediately. Front and side hits
explode but do not eliminate unless the attacker has ALL HITS. Eliminated carts are hidden and cannot steer,
move, collide, be hit again, or fire.

The last standing cart earns **one round win**, regardless of how many opponents
it hit. After 1.5 seconds the next round begins automatically, restoring carts
and scenery and clearing missiles/effects while preserving round wins. If
everyone dies in the same frame, it is a draw: restart without awarding a point.
Reaching the selected win target ends the match and displays the winner. Press
R, gamepad Start, or click RESTART MATCH to start a new match with zero wins.
There is no 60-second timer.

### Rail powerups

The first pickup appears after **30 seconds of active round play**; subsequent
pickups appear at random intervals of **5–20 seconds** on existing straight rails.
Drive over a pickup to collect it; human and bot carts follow the same rules.

- **Yellow cube — SPEED:** automatic movement is **25% faster** for 10 seconds.
- **Pink ring — ALL HITS:** your missiles eliminate opponents from **any side**
  for 10 seconds, not just from behind. It never allows hitting yourself and
  does not change the 500 ms firing cooldown or scenery/wall impacts.

Collecting the same type refreshes its 10-second duration without stacking;
both types can be active together. The HUD shows each cart's active effects and
remaining seconds. Pausing freezes drops and effects; finished-round transitions
do not advance them. Every new round or match removes uncollected pickups,
clears effects and restarts the initial 30-second delay.

Volcano uses dark basalt, warm lighting and fog, a lava river outside the walls,
small ground-level pools between rails, and three volcanoes behind the arena.
Lava is visual scenery, not a damage hazard. Neither arena changes the railway
or rear-hit rules. The selected theme survives automatic quality rebuilds.

Missiles explode on whatever they meet first: a cart, a wall, or one of the
trees that stand between the rails. A blast wrecks the scenery around it, and
each wrecked tree goes up in a smaller blast of its own a moment later, so a
shot into a thicket clears a patch of forest. Low growth such as bushes and
ferns is flown over rather than struck, but still goes up in the blast. The
forest grows back when a round starts or restarts.

## Graphics quality

There are no video settings to choose: the game adjusts its own detail to keep
the frame rate high. It measures the frame rate every second and moves along
seven quality levels, from 0 (half resolution, no shadows, lighter effects, a
sparser forest outside the walls) to 6 (full resolution, full shadows and
effects, 4x anti-aliasing), which is the game's full look.

- The target is the display cadence, capped at 60 FPS: the native build reads
  the current monitor's refresh rate, and the browser samples the empty page's
  `requestAnimationFrame` cadence **before** starting the game (normally under
  a second, with a 1.5-second deadline). Slow game FPS never lowers its own
  target. Missing, hidden, unstable or implausible calibration falls back to
  60 FPS.
- It starts at the full look, or at the level the last visit ended on (the
  browser remembers it), and steps down when the frame rate stays below
  50/60 of the target; below half the target it drops two levels at once.
  At a 60 FPS target these thresholds are still 50 and 30 FPS.
- After a few seconds at 57/60 of the target or more it tries the next level
  up. Thus 30/40 Hz displays can keep or recover full detail at their capped
  cadence. If a higher level makes the game slow again it steps back, and
  waits longer before the next try, so the picture does not keep changing.
- The seconds after loading and isolated long frames are not counted. Hitches
  recurring before a second of normal frames has passed count with their full
  durations, even when fast frames separate them; a persistently slow machine
  still steps down. Hidden browser tabs and invisible native windows suspend
  measurement rather than count throttled background frames.
- Only detail changes; nothing about how the game plays does.

**F3** shows the frame rate and the current level, e.g. `60 FPS 16.7 ms Q6/6
AUTO`. To fix the level instead, for comparing builds or taking screenshots,
open the web build with `?quality=N` (e.g. `index.html?quality=0`), or set
`RYGGATTACK_QUALITY=N` for the desktop build; the readout then says `FIXED`.

## Source layout

The code is grouped by responsibility. Start with `src/lib.rs` for client
configuration and `src/simulation.rs` for the single shared gameplay order.
`src/main.rs` is the offline client entry point.

| File | Responsibility |
| --- | --- |
| `src/main.rs` / `src/lib.rs` | Client entry point / application setup |
| `src/simulation.rs` | Shared fixed gameplay chain and separate presentation animation |
| `src/input.rs` | Device-independent held steering/fire intent and tick application |
| `src/headless.rs` | Windowless shared-gameplay harness, configuration, diagnostics |
| `src/bin/ryggattack-headless.rs` | Bounded no-display command-line runner |
| `src/game.rs` | Elimination, first-to scoring, automatic rounds, and match restart |
| `src/match_settings.rs` | Bot count, win target, and arena configuration |
| `src/arena.rs` | Forest/volcano ground, lighting, lava, and volcano landmarks |
| `src/scene.rs` | Assemble the camera, lighting, ground, tracks, forest, carts, and HUD |
| `src/scenery.rs` | Where every tree, bush, and rock stands, and how the forest grows back |
| `src/explosions.rs` | Blasts, the wreckage they make of the scenery, and the chain reaction |
| `src/players/mod.rs` | Player state, device and bot input, movement, and collisions |
| `src/players/roster.rs` | Which device holds each of the four seats |
| `src/players/visuals.rs` | Cart and rider models |
| `src/combat.rs` | 500 ms missile cooldown, impacts, and rear-hit elimination |
| `src/powerups.rs` | Rail pickups, active-round drop timing, and temporary per-seat buffs |
| `src/settings.rs` | Automatic graphics quality: the quality levels and the frame-rate governor |
| `src/ui/mod.rs` | Main menu, pause dialog, and score display |
| `src/ui/lobby.rs` | The join lobby and its seat cards |
| `src/ui/nav.rs` | One reading of the menu keys, sticks, and buttons |
| `src/tracks/map.rs` | Generate the connected rail network and grid coordinates |
| `src/tracks/path.rs` | Junction routes, smooth movement, and reversing carts |
| `src/tracks/render.rs` | Rail, track-bed, and sleeper meshes |

The web build is driven from outside `src/`: `web/index.html` is the page
shell that hosts the canvas, `scripts/build-web.sh` compiles the WebAssembly
bundle into `dist/web/`, and `web/Dockerfile` with `web/nginx.conf` packages
that bundle into the image a tag push publishes.

`src/tracks/mod.rs` exposes the track functions and types used by the rest of
the game. Tuning constants live with the feature they control. Tests live
alongside the code they check; run them with `cargo test` inside the development
environment. Use `cargo fmt` to format changes.

### Gameplay verification

`cargo test --locked` exercises the actual Bevy ECS systems for rear-hit
elimination, simultaneous hits, 499/500 ms firing boundaries, dead-cart movement,
round restoration, automatic continuation, and mouse restart. Pure tests check
bot-seat limits, solo practice, draws, win targets, and the input-agnostic lobby
settings. Arena tests rebuild quality and switch back to forest, checking that
volcano decorations remain and lava pools stay clear of rails.

```sh
cargo test --locked
python3 scripts/headless-smoke.py
cargo clippy --all-targets
node --test scripts/refresh-rate.test.mjs
scripts/build-web.sh
# Playwright must be available; PLAYWRIGHT_NODE_MODULES can point at its package.
node scripts/perf/shots.mjs dist/web volcano --width 1280 --height 720 \
  --bots 0 --target 5 --arena volcano --quality 0 --out verification
# Longer, real-input pickup playtest (software-rendered frames need more wall time):
node scripts/perf/powerups.mjs dist/web verification forest
```

`scripts/perf/foundation.mjs` additionally drives a real solo round with steering
and fire, verifies pixel-identical paused screenshots, resumes, and checks
browser errors. Run it with the same Playwright setup, passing `dist/web` and
an output directory. The foundation tests also compare 4/64 FPS frame
partitions, external-seat input and restoration, seeded pickup replay,
full-tick rear-hit scoring, exact held-fire cooldown ticks, sub-tick local fire
taps, and suppression of paused/finished input. The browser harness probes its
own static-file path boundary before starting Chromium.

The screenshot script captures lobby, selected settings, and gameplay. Use
`--arena forest --bots 1` to check the original theme and a two-cart match.
The powerup script steers a solo cart through multiple rail routes, pauses and
resumes, fires, captures successive pickup/HUD screenshots, and fails on browser
errors. It never injects pickups or buffs; inspect its images alongside the ECS
tests for timing, collection, both effects, and reset semantics.
`scripts/timed-check.py LABEL COMMAND...` stores exact output in
`verification/LABEL.log` and wall-clock timings in `verification/timings.tsv`.
The optional wasm-opt pass can be omitted for local builds by filtering its
installation directory from PATH while keeping wasm-bindgen reachable.

When using the user-local Rust environment, source
`~/.local/share/rust-dev-env.sh` in **each build command's shell**. That script
appends a native-library flag, so clear an already inherited RUSTFLAGS first to
avoid changing Cargo's cache key on every run. Worktrees must set
CARGO_TARGET_DIR and symlink their `target` to the shared target directory:
`build-web.sh` reads the WASM through the relative `target/` path.

## Near-term roadmap

1. Improve movement, hit feedback, bot behavior, and round presentation.
2. Add original character and arena art.
3. Shrink the download; 39 MB is a lot to ask of a first visit.
4. Validate Android and iOS packaging early.
