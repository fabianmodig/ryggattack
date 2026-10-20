# Phase 3 verified runtime evidence — October 20, 2026

These are actual final-source execution results, not synthetic protocol fixtures. `manifest.json` binds runtime source, test sources, browser artifacts, native binaries and result files to hashes. Historical failed/exploratory attempts are retained outside Git under `/home/hermes/.hermes/cache/scratch/ryggattack-phase3-attempts/`; they do not establish current coverage.

## Actual browser/native gate

`node scripts/network-smoke.mjs docs/online-rooms-evidence` completed successfully in **357.710 seconds** on the final runtime source, after the lobby-replica cleanup correction. Two actual Chromium/SwiftShader Bevy clients and the real native shared adapter joined the same private room. A proxy added 75 ms each direction. There were **2,497 identical canonical authoritative snapshots** at common ticks 4164–15744, no captured browser errors, and **zero server tick-overrun warnings**. Equality is observed network state at matching ticks, not a proof that every rendered entity is identical. Float canonicalization follows protocol `f32` semantics.

The 19 recorded checks include actual HTML create/join, host settings, disabled non-host controls, visible invalid settings, ready/unready cancellation, settings clearing readiness, isolated second room, real keyboard movement/fire, menu/form/blur neutral input, a natural winner returning everyone to the same lobby with settings retained and all readiness cleared, deliberate leave/host election, share-link join as a new identity, malformed-invite feedback, and offline menu/play/pause with no WebSocket connection. See `results.json` for the exact enumerated checks and per-client metrics. Browser wire-ack medians were approximately 181.5/180.6 ms; native 188.4 ms. These are not input-to-photon measurements; software-rendered HUD samples are much slower.

The lifecycle fixture selects first-to-one through the real host UI and uses the service's ordinary first-room seed 3. No winner injection or production result hook is used. The native helper remains connected through explicit teardown. Earlier first-to-three random-play attempts could stalemate when the helper exited and left a neutral cart. The harness now records full failure state and uses actual steering/fire on a surviving browser participant. Ordinary multi-round first-to-three behavior remains covered by the shared/headless tests, not claimed from this first-to-one graphical fixture.

## Separate real-service load gate

`node scripts/rooms-load.mjs docs/online-rooms-evidence/load.json` passed in **75.159 seconds** without compiler or browser contention. Twenty-second steady measurement windows exercised one room (one human, three actual bots) and the capacity of 16 rooms (16 humans, 48 actual bots). Minimum observed rates were 63.999784 and 64.000199 Hz respectively, with zero overrun warnings. This is a small local development-service measurement, not a public capacity/SLA guarantee. The 17th room was rejected.

Earlier exploratory runtime runs overlapped compilation/software rendering and logged wall-clock-debt discards. Those failures remain recorded; snapshot agreement alone was not treated as throughput evidence. The final replay records wall-clock/tick rates and retains server logs.

## Screenshot inspection

- `initial-playing-BrowserA.png` and `initial-playing-BrowserB.png`: real running arena, cart/rider/rail geometry and Running room status.
- `returned-lobby.png`: same room and host, Volcano/first-to-one/one-bot settings, all three humans not ready, Ready button; stale cart/projectile replicas absent.
- `lobby-cancelled.png`, `share-link-joined.png`, `invalid-invite.png`: actual UI states for cancellation, shared invite/new participant and visible validation feedback.
- `offline-menu.png`, `offline-playing.png`, `offline-pause.png`: existing local flow; pause overlay remains present.

Online scenery is still simple collider proxies rather than offline forest/volcano visual parity. The diagnostic native-style HUD is crowded at 640×480 and contains unsupported Unicode glyphs. This is functional development UI, not finished visual polish.

## Final checks

| Gate | Seconds | Result |
| --- | ---: | --- |
| `cargo test --locked --all-targets` | 43.144 | **161 passing**, including 15 malformed-lobby/identity cases and explicit election assertion |
| `cargo clippy --locked --all-targets` | 1.079 | Passed; four pre-existing warnings |
| `cargo build --locked --bins` | 0.413 | Passed on final runtime source |
| Release web build plus optimization | 184.275 | Passed; rounded WASM size 53 MB to 39 MB |
| Actual browser/native replay | 357.710 | All 19 checks passed |
| Separate service load | 75.159 | One/16 rooms, approximately 64 Hz, no overrun warnings |
| Raw socket-abuse harness | 10.548 | All 16 checks passed |
| Real headless rules smoke | 1.264 | Passed, including completed three-win match |

`rooms-ui.test.mjs`, `server-banner.test.mjs` and `git diff --check` passed too. The test-only review additions were exercised after the runtime replay without changing runtime source. Gate logs and the complete timing history are under `logs/`.

## Review and limitations

Two independent read-only reviews found no blocking backend or client/lifecycle defect. Their coverage suggestions were addressed by making host-election integration failure explicit and adding malformed-lobby/local-identity rejection cases. These are test-only changes after the replay; no runtime source changed.

Native graphical startup was attempted but this host has no DISPLAY/Wayland server. Native UI has windowless ECS coverage and the graphical binary builds, but an actual native graphical session remains manually unverified. OS-window blur is also unverified: Playwright focus emulation permits only the real canvas-blur test here.

This remains loopback-only and unauthenticated. Reconnect credentials, reservations, bot takeover, last-human pause/resume and 60-second retention are phase 4, not implemented here. Disconnected running characters receive neutral input and can cause stalemates. Empty rooms are removed immediately. Solo zero-bot practice remains offline, with an explicit online validation message. No backend deployment or infrastructure change was made.
