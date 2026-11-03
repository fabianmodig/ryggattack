# Multitouch sampled-fire cancellation repair

This report supersedes **only the cancellation acceptance** of `c65a509d38decf37ee23850b2dc9122808a8d4e1`.
The original `docs/mobile-touch-evidence/` screenshots, manifests and replay logs
remain byte-identical historical evidence. No push, publication or deployment
was performed. The corrected candidate still requires independent review.

## Defect and narrow repair

The old second-pointer branch cleared the JS fire bit but did not change the
bridge epoch. If Rust had already sampled a tap, local fixed-tick pending fire
and online pending-send fire survived. `createGesture` now notifies the bridge
of multitouch cancellation, advancing the epoch without clearing its active
finger set. Partial lifts and replacement fingers cannot enable new gestures
until every participating finger lifts. Full resets retain their old semantics.
Only `web/touch-controls.mjs` changes production runtime behavior; no Rust,
Cargo, build-script, asset, camera, protocol or focus-policy changes were made.

## Regression and execution evidence

- `02-js-red.txt`: genuine pre-fix TDD failure after tap **already sampled**;
  cancellation returned epoch 1 instead of 2 (0.170 s, exit 1).
- `03-js-green.txt`: same regression green with the minimal runtime repair
  (7 touch tests, 0.186 s). Partial lifts, a replacement finger, and fresh
  single-finger input after all releases are covered.
- `04-adapters-green.txt`: expanded touch suite (8 tests, 0.724 s). It feeds
  real JS bridge samples into compiled, unmodified source blocks extracted
  from `sample_touch`, local `human_input` and the online pending-send adapter.
  A sampled tap first sets **both** pending states while transport is throttled;
  multitouch then clears both; the next send has `fire=false`. Neutral physical
  device and transport fixtures isolate these transitions; this is not a full
  Bevy online system or real network transport test.
- `07-source-real-red.txt`: the expanded source-real fixture rerun with archived
  **old** JS fails because fixed-tick pending fire survives. This diagnostic
  corroborates the original TDD failure; it is not relabelled as pre-fix TDD.
- `12-node-all-selected.txt`: **168 selected Node feature/UI tests**, all pass
  (2.268 s), including calibration/startup, touch, HUD/assets and harness tests.
- `10-native-rerun.txt`: **257 native tests**, all pass (42.292 s). In particular,
  `touch_intent_reaches_local_fixed_input_and_clears_pending_on_cancel` runs the
  actual human-input and fixed-input systems, and the real-simulation touch
  cooldown regression passes. Test profile cache preparation: 0.26 s, as
  recorded by Cargo; no WASM compiler rebuild was requested for this repair.
- `11-clippy.txt`: `cargo clippy --locked --all-targets` passes (0.584 s), with
  the same seven pre-existing warnings. One Cargo job was configured locally.
- Room UI, redirect, chart-version, retained prediction-evidence/verifier,
  container-network/build contracts, Helm/chart, native TLS, server banner,
  production policy/socket/abuse, and the real headless executable all pass;
  individual commands, timings and outputs are retained here. Raw assertion/native
  logs preserve original whitespace; narrowly named `.gitattributes` exceptions
  apply only to those immutable evidence logs, never implementation files.

### Preserved interruptions and intermittent native failure

`01-js-red.txt` is a **launcher failure**, not TDD: `/usr/bin/time` was absent.
The actual RED execution is `02-js-red.txt`.

`05-native.txt` preserves an initial failure in the unchanged
`net_client::resilience_tests::frame_arrival_time_survives_a_late_adapter_poll`
(209 passed, 1 failed in the library; full command exited 101 after 6.615 s).
That test connects a real client to loopback port 9 before manually injecting a
frame; asynchronous connection failure may race the injection. This cause is
not independently proven. `native-failure-source-equivalence.json` verifies the
entire module byte-identical to c65a509. Five exact focused reruns pass, followed
by the full 257-test pass. No unrelated source was changed to suppress this
intermittent failure. The original `&&` chain did not run Clippy; the separately
recorded successful Clippy command did.

`28-headless-interrupted.json` records a later optional gate invocation stopped
by the outer 180-second tool budget. The helper's default `cargo run` path had
repeatedly sourced flags and could invalidate caches; no failure result or build
completion is inferred from its empty buffered log. No live Cargo/rustc child
remained on inspection. `29-headless-direct.txt` instead passes using the existing
unchanged-source executable directly (1.258 s), including real solo/match,
movement, fire, pickup/destruction and invalid-argument assertions.

## Corrected browser replay and compiler provenance

`browser/results.json` is a fresh **14-check**, **375 × 740 Chromium mobile**
replay (133.730 s, no page errors), executed by the exact hash of the committed
`mobile-touch-smoke.mjs`. All eight new screenshots were opened and visually
inspected: portrait arena/rails/carts fit, menus and touch controls are readable,
Resume and Restart are present at their barriers, and the online name is the
synthetic `TouchQA`. No credentials or personal data are visible. The smaller
legacy canvas lobby text is still small; full-size DOM touch controls remain
available. Still images alone do not prove cancellation state.

The fresh offline and online runs each trace an actual WASM-consumed `fire=true`
sample followed by real trusted multitouch and a larger cancellation epoch.
Subsequent samples are neutral. Offline real ECS steering becomes neutral and
cooldown drains to zero; online actual WebSocket sends remain neutral. Real
local cooldown and authoritative online cooldown establish firing separately.
The deterministic source-real regression, not browser scheduling luck, proves
cancellation **before** the next fixed tick/send. A cancellation cannot retract
a shot already consumed or transmitted before the second finger arrived.

`source.json` binds all runtime/compiler inputs and executed regression/harness
sources by hash. `artifact-receipt.json` explicitly describes the **mixed bundle**:

- Reused unchanged historical release WASM and wasm-bindgen JS, matching the
  old browser receipt byte-for-byte. All historical Rust/Cargo/build-script
  inputs and all assets remain unchanged relative to c65a509.
- Restaged only the corrected `web/touch-controls.mjs` into `dist/web/`.
- Original compiler/build log and original manifests retain their own hashes
  and provenance. **This was not a new full WASM build.** No current compiler
  version is invented for the historical build. The historical optional
  `wasm-opt` omission is unchanged.

`verification.json` records counts, screenshot hashes/review and timings.
`evidence-manifest.json` binds the retained proof files and runtime/test sources;
its staged bytes were checked before committing.

## Remaining boundaries

Physical iPhone/iPad and Apple Safari remain **unverified**, including real
safe areas, OS-window blur, mobile keyboard/address-bar behavior and GPU
performance. The online protocol is still held-state input: neutral cannot undo
an already delivered fire intent, and a stalled connection may permit repeats
at normal cooldown until neutral arrives. Native graphical interaction,
container image publication/deployment and optimized-WASM replay were not run.
Independent parent review is required before any publication.
