# Online movement: analysis and implementation contract

Base: Forgejo `origin/main` a48fbf5 (freshly fetched for t_4de42a24).
No production service, public room, merge or deployment is part of this work.

## Existing path / causes to measure

* Shared gameplay runs at 64 Hz (`src/simulation.rs:5-37`). Server submits the latest held input before stepping once and publishes every fourth tick (`src/server.rs:673-855`): nominal snapshots are 16 Hz / 62.5 ms, not 64 Hz.
* Input already has sequence numbers. `src/net_client.rs:653-695` sends at most one unacknowledged message and no faster than 62 ms. The acknowledgment is the latest *received* sequence included after the simulation step (`src/server.rs:708-720,825-854`), not a count of client prediction ticks. Multiple messages may overwrite intent before one step, and one message can be held for many steps. Therefore replaying once per message, or replaying a held message once per render, is incorrect.
* `src/online.rs:490-495,670-688` waits for the authority for **all** carts, then lerps the previous/current world positions over a hard-coded 62.5 ms. It adds a full snapshot interval to input latency, resets its interpolation clock at renderer consumption rather than socket arrival, cannot traverse rail bends correctly and holds after the interval under jitter. Mailbox coalescing can skip intermediate samples. This is separate from fixed-tick offline presentation and from slow SwiftShader rendering.
* Wire cart state (`src/headless.rs:41-51`) has position/direction and buff timers, but no directed rail path/progress/requested turn. A shared-rule predictor cannot reconstruct a unique route from a world position at a junction. Add authoritative rail state rather than guessing.
* Server authority for contacts, combat, pickups and round progression is retained. Prediction must never execute those systems or emit events.

## Measured baseline

`node scripts/prediction-baseline.mjs` exercised an isolated loopback backend and two real WebSocket peers, no browser. Across 36 snapshots it measured 63.9902 server ticks/s, exactly four ticks per snapshot, snapshot gaps 62.166–62.840 ms (mean 62.510 ms). Twelve sampled input changes reached snapshot acknowledgments in 32.323–59.353 ms (mean 36.879 ms). This run does not exercise the stop-and-wait adapter or rendering, so it cannot measure graphical latency or claim a browser frame rate. The measured server was not running slowly; the code-level extra interpolation interval and RTT-gated input remain distinct causes. Raw sanitized results: `verification/prediction-baseline.json`.

Baseline `cargo test --locked` passed (259.579 s including compilation). The selected 163 Node tests passed (1.158 s). These are pre-implementation results, not the feature gate.

## Design decisions

1. Add explicit authoritative rail state to snapshots and extract a pure cart-travel step shared by `move_players` and prediction. Keep input normalization and buff expiry ordering identical. Never accept client positions.
2. Use tick-indexed actual prediction steps, separate from input sequence/acknowledgment. Acknowledgments do not mean one message == one tick. Reconciliation discards steps covered by the authoritative simulation tick and restores authoritative route/speed/liveness, then replays only recorded later steps with their actual sampled intent; never synthesize steps to match message counts. Bound lead/history/catch-up, reject stale frames and reset when a timeline cannot be reconstructed. Input delivery must not wait an RTT before transmitting a changed held intent.
3. Keep authority frames/HUD untouched. Local prediction and presentation own separate state. Small correction residuals must use directed rail distance, not a world-space lerp; incompatible routes, reversal, death/respawn, teleports and barriers snap and reset. Local render interpolation is between adjacent fixed states.
4. Remote presentation uses a bounded arrival-time/tick buffer with explicit delayed clock, no indefinite extrapolation, and rail traversal only when endpoints prove a compatible route. Non-cart objects keep their current authoritative timing.
5. Welcome/non-running/coalesced barriers, identity change, loss of connectivity, long stalls and rematches clear input history, clock estimates, corrections and remote samples. Focus/menu suppression remains in the real input path. Anonymous bearer reconnect is unchanged and is not theft-resistant.

The exact compatibility version and clock accounting must be settled and exercised in RED/GREEN tests before integration. This document is a contract, **not evidence that those features are implemented**.

## Acceptance matrix (unverified until linked results exist)

| Area | Required evidence | Status |
|---|---|---|
| Baseline | Real socket ticks/snapshot cadence and input-to-ack timing; separate render limits | Pending |
| Shared travel | Pure step agrees with actual shared FixedUpdate, including speed expiry, rail seams and turns | Pending |
| Accounting | Multiple inputs/tick, held input/multiple ticks, ack jumps/duplicates/stale frames, exact replay of actual steps | Pending |
| Bounds | History/lead/catch-up/remote buffer bounded; stalls/background safe fallback | Pending |
| Corrections | Route disagreement, reversal, contact displacement, speed change, death/respawn; no irreversible replay | Pending |
| Presentation | Local between-tick interpolation; remote delay/jitter and rail bends; render-rate-independent correction decay | Pending |
| Lifecycle | Real adapter/scheduler barriers, identity, reconnect, lobby/countdown/running/rematch; focus suppression | Pending |
| Regression | Rust/Clippy, all Node/Helm CI gates, offline same-artifact browser interaction | Pending |
| Browser | Two isolated Chromium processes, normal menus/keyboard, zero and added latency/jitter, quantitative movement before confirmation and convergence | Pending |
| Provenance | Source revision, JS/WASM hashes, build durations, sanitized evidence and independent review | Pending |

## Execution constraints

One Cargo job/build at a time, host-local `.cargo/config.toml`, shared target cache. Live initial host memory: 12 GiB; worker/visible ancestors report `memory.max=max`, no cgroup OOM events. Disk initially 37 GiB available. Do not change limits or install packages. Build timings and failures remain in `verification/`. Forgejo PR CI is the publication gate; main release would publish paired containers and update the deployment chart, so main must not be changed. GitHub Pages only publishes a migration redirect.
