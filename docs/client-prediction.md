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

## Native implementation checkpoint — not acceptance-complete

Current implementation is preserved on `feat/client-prediction-t4de42` in this task's `repo` worktree. The online update path now advances the bounded local movement model and supplies separate local/remote poses to the real replica renderer; it no longer uses world-space cart lerps. Welcome/identity/topology/non-running boundaries clear that model. Reconnect/admission uses the transport's neutral-release suppression gate. No WASM artifact or graphical replay has been produced for these changes.

Compatibility decision: keep protocol 4 with additive optional `Frame.motion`. Legacy servers omit it and the new client falls back to authoritative presentation; old clients ignore the additive field. Motion includes authoritative directed rails and per-seat held control `(sequence, since_tick, axes)` selected at a completed simulation boundary. Existing `Frame.ack` remains a delivery watermark. Multiple delivered sequences may collapse before a step; unchanged held input may cover many steps. Server-generated neutral/direct submissions use applied sequence zero. No client position crosses the authority boundary.

Prediction records actual 64 Hz local travel steps separately from transport packets, restores snapshots and replays the recorded suffix beyond their simulation tick. Its tick correspondence is speculative, not proof of exact cross-machine input application time. The current implementation needs additional asymmetric-latency/clock-alignment, missing-history and batching scrutiny before acceptance. There is no general combat rollback. Local corrections within the same directed path and within 0.5 rail units decay with an exponential 0.1-second time constant; route/lifecycle disagreements snap rather than cut diagonally. Remote presentation holds a 16-frame buffer, uses callback arrival timestamps, a monotonic delayed tick cursor and bounded 125–250 ms jitter-adjusted delay, with no indefinite extrapolation. A render delta above 0.5 seconds or more than 32 speculative ticks falls back to authority.

### Verified at this checkpoint

* Full `cargo test --locked`: **243 tests passed**, including 201 library tests and 12 dedicated prediction integration tests; **235.820 s**, including compilation.
* `cargo clippy --locked --all-targets`: exit 0, **8.513 s**, with warnings; baseline-aware warning comparison remains pending. One new test style warning (`manual_is_multiple_of`) is known and not hidden.
* Selected **163 Node tests passed** in **1.197 s**.
* Actual local native TLS, banner, production policy, isolated production-mode smoke, 16 raw-socket abuse checks and headless executable smoke all passed. Their logs and measured durations are in `verification/prediction-current-*.log` and `verification/timings.tsv`.
* Every RED attempt and initial compile failure is retained unchanged, including the initially mislabeled `prediction-travel-green.log` that contains a passing travel test but the newly added failing reconciliation test. The final complete suite supersedes these attempts; historical failures are not rewritten.
* `scripts/prediction-checkpoint.py` executed successfully, scanned the selected text logs for private material and emitted `verification/prediction-checkpoint.json` with exact tested runtime hashes, evidence hashes and remaining acceptance work. This is a native checkpoint manifest, not a complete browser evidence verifier.

### Continuation work — required before PR/reviewer handoff

1. Inspect the implementation against the full card matrix; strengthen real adapter/shared-schedule tests beyond the initial update-helper and ECS-renderer coverage. Test speed expiry/corrections, genuine collisions, route reversal/disagreement, missing suffixes, ack anomalies, seat/room/rematch and long-clock stalls. Review overflow and malformed optional-motion metadata handling. Consider explicit round identity if phase/gap guards cannot establish lifecycle safety.
2. Characterize tick alignment under asymmetric delay/jitter/batching; do not relabel local estimated ticks as established authoritative correspondence. Add precise measured input/render/correction diagnostics without exposing credentials. Baseline currently measures transport only, not graphical performance.
3. Finish all CI gates, compare Clippy with fresh base `a48fbf5`, remove introduced warnings and rerun changed tests. Helm/chart Python gates have not run in this attempt.
4. Freeze runtime, build WASM with one Cargo job, record source/artifact hashes, then exercise two actual rendered clients in separate Chromium processes with normal menus/keyboard at zero and bounded additional latency/jitter. Build a non-serial chunk delay fixture; verify natural junction/contact/respawn and lifecycle behavior quantitatively. Run offline graphical regression on the same artifact after closing the concurrent renderers. Native graphical testing is still deferred.
5. Preserve scanned evidence, prepare the focused Forgejo PR, verify exact-head CI, and request native same-card review from `reviewer`. Do not merge, deploy, restart production or access public rooms.

Playwright is already present at `/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json`; Chromium caches exist. The existing `scripts/unified-menu-smoke.mjs` provides real menu selectors, separate browser processes, safe loopback static serving, authoritative keyboard/lifecycle checks and artifact hashing, but is not a prediction acceptance harness.

## Execution constraints

One Cargo job/build at a time, host-local `.cargo/config.toml`, shared target cache. Live initial host memory: 12 GiB; worker/visible ancestors report `memory.max=max`, no cgroup OOM events. Disk initially 37 GiB available. Do not change limits or install packages. Build timings and failures remain in `verification/`. Forgejo PR CI is the publication gate; main release would publish paired containers and update the deployment chart, so main must not be changed. GitHub Pages only publishes a migration redirect.
