# Phase 2 verification evidence — October 20, 2026

This is the original phase-2 implementation replay. It passed against the
runtime sources and artifacts in [manifest.json](manifest.json), but those
sources predate the later security-review fixes. The [review replay](../online-network-review-evidence/README.md)
binds the newer sources to their own native tests, web build, and browser/native
runtime result. [results.json](results.json) here is the actual historical
browser/native run result, not synthesized data.

## Final gates

All commands ran in the feature worktree, with no concurrent Cargo builds.
Local environment setup before Rust/runtime commands:

```sh
unset RUSTFLAGS LIBRARY_PATH
source ~/.local/share/rust-dev-env.sh
export CARGO_TARGET_DIR=/home/hermes/src/ryggattack/target
```

| Command | Wall seconds | Actual result |
| --- | ---: | --- |
| `cargo test --locked --all-targets` | 99.247 | 139 passing: 130 library, 2 protocol/replication, 2 real-socket, 5 shared-simulation |
| `cargo clippy --locked --all-targets` | 3.390 | Success; four existing warnings in unchanged explosions/settings/UI code |
| `cargo build --locked --bins` | 0.421 | All native executables built, including graphical client/server/probe |
| `scripts/build-web.sh` | 187.465 | Release WASM + matching wasm-bindgen + wasm-opt; 53 MB to 39 MB (script's rounded sizes) |
| `node scripts/network-smoke.mjs dist/network-evidence-complete` | 195.504 | Full two-rendered-browser/native crossplay and subsequent offline smoke passed |
| `python3 scripts/headless-smoke.py target/debug/ryggattack-headless` | 1.296 | Real no-display missiles/destruction/drop, solo rules, complete one-human/one-bot three-win match and reset |
| `node scripts/network-sockets.mjs` | 1.359 | All 10 socket-abuse/recovery checks passed |
| `node --test scripts/refresh-rate.test.mjs` | 0.145 | Passed |
| `cargo fmt --all -- --check`; `git diff --check` | not timed | Passed |

Browser invocation also set:

```sh
export PLAYWRIGHT_NODE_MODULES=/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json
```

The test launched actual Chromium/SwiftShader, the real development server,
and `ryggattack-net-probe <delayed-address> 110`. No mocked simulation or
protocol responses were used. The manifests include native binary hashes,
all web artifact files, runtime source hashes and exact tool versions.

## Actual crossplay and delay results

* Browser A, browser B and native probe received server seats **0, 1 and 2**
  (displayed as players 1, 2 and 3).
* **1,754 common authoritative snapshots**, ticks **2480–9492**, had identical
  canonical full-state hashes across all three clients. Canonicalization
  respects declared `f32` precision: the native probe's `serde_json::Value`
  prints promoted floats with more digits than direct protocol serialization.
  Integer IDs/ticks are preserved. This is state replication, not lockstep
  determinism or identical JSON text.
* Real browser keyboard steering and firing reached the socket. All clients
  observed movement, turns, missiles, explosions, elimination and round scoring;
  the server/native seat won one round, yielding `[0, 0, 1, 0]` on all clients.
  The run did not complete the full online first-to-three match. Counts named
  `*_observations`/`*_frames` are observations across snapshots, not unique shots
  or unique elimination events.
* A bounded TCP proxy added **75 ms each direction**. Browser input-to-ack wire
  medians were **182.540 ms / 179.305 ms**, with p95 **208.789 ms / 207.702 ms**.
  Native probe frame-sampled acknowledgement median was **188.186 ms**; its
  final status remained online. No server tick overruns were logged in this run.
* Menu input suppression, editing the HTML form without stolen focus, and a
  real canvas blur while Space was held produced neutral input while server
  ticks continued. The actual OS-window-blur path is not graphically verified:
  Playwright forces headless pages focused, so `bringToFront` is not a blur.
  The native predicate test covers the window-focus gate.
* Offline launch created one top-level canvas and no iframes/WebSockets. Real
  controls entered the ordinary forest game and opened the existing pause
  dialog on this same WASM artifact.

The wire medians are **not** a claim of equivalent perceived/input-to-photon
latency. The software-rendered HUD showed sampled ack values 866 ms and 438 ms
in the selected screenshots; render/input sampling adds significant delay.
Stop-and-wait input and no prediction are deliberate prototype limitations,
not a finished Internet responsiveness policy.

## Reviewed screenshots

* [Browser A](BrowserA.png): existing cart/rider/rail geometry, seat 1, server
  score `[0, 0, 1, 0]`, active speed timer and replicated pickup/effect proxies.
  The open entry form is the form-focus test, not a second local seat.
* [Browser B](BrowserB.png): seat 2 and the same server score/map. Screenshots
  were taken sequentially, so their displayed ticks differ; common-tick hashes,
  not screenshots at different times, establish shared state agreement.
* [Offline menu](offline-menu.png), [ordinary play](offline-playing.png), and
  [offline pause](offline-pause.png): unchanged forest/carts and visible PAUSED
  overlay. The entry form is additive to the existing page shell.

Online scenery is intentionally simple green collider proxies, not the offline
forest/backdrop. The diagnostic HUD is crowded at 640×480 and its unsupported
Unicode punctuation appears as missing glyphs; words/controls remain readable.
No native graphical window was run: this host's real native crossplay evidence
is the shared-adapter probe, plus a successful graphical-client build.

## Observed failures, root causes and repairs

The interrupted agent's observed protocol/replication/socket/adapter RED → GREEN
history was inspected before preserving its code. The resumed initial native
suite passed, but actual browser play exposed failures that unit success did
not cover:

1. Hello could expire while Bevy was initializing. A real-socket regression
   delaying the first pump six seconds failed, then passed with a bounded
   ten-second Hello grace and a deadline beginning at actual Hello send.
2. A longer startup still opened the transport too early. The eleven-second
   delayed-first-pump regression failed, then passed after moving socket open
   to the first poll rather than the constructor.
3. A fresh acknowledged state consumed after a six-second render stall was
   incorrectly classified as timeout. The test failed with `Server timeout`,
   then passed when only genuinely pending admission/acknowledgements time out.
4. Second-browser traces showed its first GPU submission blocking the JS event
   loop before WebSocket callbacks could run. A failing presentation-ready
   regression preceded the two-initial-frame warmup gate. Both real browsers
   subsequently connected and played together.
5. The initial delay proxy accidentally imposed a packet-size-dependent
   throughput cap. It now delays chunks concurrently, with bounded queued
   bytes and stream backpressure. The measured ~180–190 ms acknowledgements
   reflect the injected two-way delay without that artificial bandwidth cap.
6. The attempted window-blur test failed because Playwright explicitly enables
   focus emulation for every page. This was confirmed in installed primary
   Playwright source and a browser focus probe. The final test exercises actual
   canvas blur and documents the remaining OS-window coverage gap instead of
   weakening the neutral-input assertion.
7. Crossplay then passed **1,755** shared snapshots, but the *whole* smoke failed
   at offline `locator('canvas').focus()`. This partial positive evidence was
   preserved, not reported as a complete pass. The separate lifecycle diagnostic
   ([actual event record](offline-diagnostic.json)) found the correct URL/title,
   no iframe, one visible already-focused canvas, no crash/navigation/error,
   and a working offline menu screenshot. DOM queries took ~11 seconds with
   three simultaneous software-rendered games; focus exceeded 30 seconds.
   After closing the two online pages, the same offline page's focus completed
   in ~4 ms. The harness now finishes the required concurrent **two** rendered
   client/native checks, closes those renderers, then runs the independent
   offline gate. It records lifecycle/errors and waits for an actual menu
   screenshot, not an arbitrary extra sleep. The affected **full** network plus
   offline run was replayed and passed in 195.504 seconds.

Raw attempts remain locally in `/home/hermes/.hermes/cache/scratch/network-*.log`
and ignored `dist/network-evidence-*`; the failed offline stage is in
`network-browser-pass.log`. The final full run is `network-browser-complete.log`.
The committed selected evidence excludes large raw runtime logs.

### Build / test-build history during this resumed task

These timings include compilation and the named test execution where applicable.
No build profile or system service was changed to obtain a pass.

| Stage / command | Seconds |
| --- | ---: |
| Initial resumed `cargo test --all-targets` | 74.872 |
| Focus-gating RED `cargo test --lib menu_blur_and_form_focus` | 3.428 |
| Focus-gating GREEN, same command | 12.176 |
| First `cargo build --locked --bins` | 24.704 |
| First complete `scripts/build-web.sh` | 410.577 |
| Slow-start RED `cargo test --test server real_sockets` | 17.858 |
| Slow-start GREEN `cargo test --test server` | 38.780 |
| Following `cargo build --locked --bins` | 0.676 |
| Second complete web build | 184.350 |
| Deferred-open RED `cargo test --test server real_sockets` | 22.754 |
| Deferred-open GREEN `cargo test --test server` | 45.532 |
| Late-ack RED `cargo test --test server real_sockets` | 25.075 |
| Intermediate all-target GREEN tests | 89.996 |
| Third complete web build | 188.715 |
| Initial-presentation RED `cargo test --lib socket_polling_waits` | 2.281 |
| Final all-target GREEN tests | 99.247 |
| Final explicit native build | 0.421 |
| Fourth/final complete web build | 187.465 |

A preliminary focus-test invocation found Cargo absent from that tool shell's
PATH; sourcing the documented environment fixed the invocation before the
observed source-code RED. The prior agent's provider HTTP 403 was an unrelated
model-provider failure, not an application build/runtime diagnosis.

## Remaining product and operational risks

See [the scope/limits document](../online-network-slice.md). In particular:
loopback only, four pre-active seats, default match configuration, no rooms,
readiness, reconnect/reservation, bot substitution, public TLS/origin policy,
empty-room lifecycle, production packaging/deployment, prediction, full visual
parity, or asset-free server extraction. Issue #6 is not completed by this
slice. No push, merge or deployment was performed.
