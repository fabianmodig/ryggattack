# Shared simulation foundation — issue #6, milestone 1

This is a **working shared-gameplay/headless harness**, not an online release,
not a room service, and not the completed presentation extraction. Refs #6.
The issue's online requirements are unchanged.

## What is shared now

The native/WASM client and the windowless executable link the same library.
`src/simulation.rs` installs one ordered `FixedUpdate` chain containing the
existing gameplay systems:

1. Apply device-independent held intent; run the existing bot AI.
2. Tick/drop powerups; move carts on the procedural rails; reverse collisions.
3. Collect pickups; fire and move missiles; resolve scenery impacts and rear hits.
4. Run destructible-scenery detonation chains; resolve wins and round transitions.

No second set of movement, combat, powerup, or scoring rules was written.
`scene::setup_gameplay` shares the existing CPU-side cart/forest/missile setup.
Headless arena selection uses the existing forest/volcano collider builder,
including the volcano's lower basalt props. Round restoration still goes
through the original `RoundReset` and forest rebuild. Detonation processing
currently requires scenery parents with `Children`; preserving the existing
factory is deliberate, so headless destruction cannot silently skip props.

The explicit tick is **64 Hz / 15,625 microseconds**. This exactly representable
interval also divides the existing 500 ms cooldown into 32 ticks. This is a
simulation choice for this milestone, **not** a committed network send rate.

### Input boundary

`input::ControlIntent` contains two steering axes and held fire — never a
position, hit, or score. Local keyboard/gamepad polling only publishes intent;
it no longer changes cart state. The fixed chain applies it. A local sampled
fire tap is retained until one tick consumes it, even if press and release
arrive in separate render frames before that tick. Paused/finished/dead/empty
local input is neutralized instead of buffering actions into resumed play.
The external API still replaces held intent; it is not a remote action queue.
A separate
external-seat mask lets the headless caller own a seat without inventing a
keyboard/gamepad entity. Neutral intent keeps the seat human-controlled; it
does not activate bot AI. Bots fill free seats using the same roster policy
as offline play. External seats survive round restoration.

`headless::Simulation::submit` rejects out-of-range/uncontrolled seats,
non-finite axes, and axes outside -1..=1 before mutating the intent buffer.
This is an **in-process boundary**, not authentication: a future transport
must bind an authenticated participant to exactly one seat and enforce input
sequence/freshness rules. Held intent persists until replaced, including
across rounds; callers must explicitly submit neutral intent when suppressing
controls. No disconnect/menu/remote ownership policy is implemented here.

### Presentation and local pause

Menu/device polling runs in `PreUpdate`, after Bevy device and UI focus updates,
so pause can gate the following fixed ticks rather than wait for a render-frame
`Update`. Particle/debris/scorch animation and HUD work remain in `Update`.
The regression suite checks identical one-second cart results with four versus
64 render frames, and checks that the existing local pause gates movement and
powerup time. Client fixed time still follows Bevy virtual time, including its
existing maximum-frame-delta clamp; this does not promise wall-clock catch-up
through a suspended browser tab.

## Run the real executable

```sh
cargo run --locked --bin ryggattack-headless -- --seed 42 --ticks 2048 --bots 0 --fire
python3 scripts/headless-smoke.py
# Or exercise an already built executable, with no cargo invocation:
python3 scripts/headless-smoke.py target/debug/ryggattack-headless
```

The executable starts one external seat and the selected 0–3 bots, advances a
bounded number of ticks as fast as the CPU permits, then exits. It does **not**
listen on a socket or sleep to real time. It prints JSON-lines diagnostic
snapshots initially and once per simulated second, plus the final partial
second. The smoke test removes DISPLAY/WAYLAND_DISPLAY, parses actual output,
and checks moving carts, real missiles, scenery destruction, the 30-second
pickup drop, solo non-scoring, a natural one-human/one-bot match through three
wins and automatic round restoration, and invalid CLI arguments.

For another Rust caller, construct `headless::SimulationConfig`, call
`Simulation::new`, replace held controls with `submit`, advance exactly one
tick with `step`, and inspect `snapshot`. Configuration supports external
seats, bots, seed, arena, and the same 1/3/5/7 win targets as local play. The
seed controls both map generation and headless powerup RNG; resets preserve
the RNG sequence. The replay test checks actual state through the first drop
at tick 1920. This is reproducibility in the tested build, not a cross-platform
floating-point determinism guarantee or a lockstep networking design. The
existing RNG uses `usize` indexing, so native/WASM seed agreement must not be
used as a future substitute for authoritative map/state replication.

`cargo run` still selects the ordinary offline client. The web script explicitly
builds only `--bin ryggattack` now that the package contains two executables.
No dependency or Cargo profile was changed to achieve headless execution.

## Honest remaining extraction work

- This remains one Bevy package compiled with its existing rendering features.
  The harness starts **no render/window/device plugins and needs no GPU**, but
  still allocates CPU mesh/material assets and visual components. Gameplay
  spawn/reset/detonation systems still construct visual entities. The harness
  runs presentation cleanup after each fixed tick to keep those lifetimes
  bounded. This is **not yet an asset-free server crate**.
- Authoritative entities still use Bevy `Transform` and `Visibility`; `Player`
  retains the offline `InputSource` field. The external-seat bridge removes
  the need for fake devices, but is not a complete participant/control model.
- `Snapshot` is explicitly diagnostic: it omits full rail traversal, projectile,
  pickup, scenery, RNG, and event state. It is not a save, reconnect, or network
  replication schema. Seat indices are not stable server-generated identities.
- Headless startup is explicit and tested; it is not a wall-clock server runner,
  room allocator, pause/reservation manager, or container delivery path.
- No transport, room/lobby protocol, smoothing/prediction, reconnect credentials,
  disconnect bots, online menu behavior, multi-room service, deployment, or
  infrastructure changes are included.

The next extraction should turn visual construction into client-consumed
presentation events/projections, retain gameplay-relevant scenery state on the
authority, and expose a complete snapshot/event model before transport is added.
The fixed chain is the reuse point; adding a separate simplified server game
loop would defeat this milestone.

## Verification and review focus

Observed failing tests were recorded **before** their implementation for:
input polling mutating authoritative state, gameplay running in `Update`,
external intent overwritten by bot AI, external seating lost on reset,
missing headless executable, invalid/uncontrolled input being accepted,
invalid match settings being accepted, wall-clock-seeded powerup replay,
headless arena selection being ignored, and the explicit runner leaving
`Time<Fixed>` at zero while generic time advanced. The runner now advances
the fixed clock and installs its generic view, matching Bevy fixed-schedule
clock semantics. Existing-rule characterization tests
also cover exact cooldown ticks, the full rear-hit → match-scoring chain,
render-cadence invariance, and local pause. Late self-review added observed-red
regressions for a sub-tick local fire tap being lost and paused/finished input
being buffered; both now pass. The new browser harness also has a real HTTP
path-boundary regression (encoded traversal was served before the guard),
with traversal and malformed-path rejection checked before Chromium starts.

Re-run:

```sh
cargo test --locked
cargo clippy --locked --all-targets
cargo fmt --all -- --check
cargo build --locked --bins
node --test scripts/refresh-rate.test.mjs
python3 scripts/headless-smoke.py
scripts/build-web.sh
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/perf/foundation.mjs dist/web verification/browser
```

Review the schedule move particularly carefully: device/UI intent must run
before the fixed loop, deferred commands must remain visible between chained
combat/reset systems, pause must stop every tick in the frame, and restart
must remain a frame-edge operation (not be repeated by catch-up ticks).
Gameplay now uses small discrete steps rather than render-sized deltas;
missile/collision timing will be more finely sampled. Remote interpolation is
not implemented, so high-refresh displays may expose fixed-step cart motion.

Final native/headless verification passed **126 Rust tests**, both native
binaries, and the latest full release web build including wasm-opt. The
no-display runner naturally completed a one-human, one-bot match through three
wins and automatic round restoration.

Full Chromium SwiftShader play in forest/volcano, solo pause/resume, and two
independent keyboard seats covers source commit
`3f452b169dc0cecb459da363705ac38473513f4a`. The later local tap/pause fixes in
`src/input.rs` and `src/players/mod.rs` have the complete native/headless tests
and fresh WASM build, but **were not graphically replayed afterward**. Review
that narrow post-playtest delta explicitly; the evidence index and results
metadata preserve the exact source/artifact boundary.

See the [committed evidence index](online-foundation/evidence/README.md) for
exact commands/timings, observed failures, actual logs/state changes, screenshot
review, and remaining runtime coverage gaps. Full original run logs and timings
are collected by `scripts/timed-check.py` in the ignored `verification/` directory.

An initial foreground baseline attempt exceeded the 420-second tool window without a
completed test result. The subsequent tracked uncapped baseline build hit an
8 GiB/no-swap OOM; the clean baseline retry with two build jobs passed. The
parent then increased the host allocation to 12 GiB. No
production build-profile workaround was introduced. Strict `clippy -D warnings`
finds four pre-existing warnings in unchanged `explosions.rs`, `settings.rs`,
and `ui/mod.rs`; ordinary all-target clippy passes with those same warnings.
