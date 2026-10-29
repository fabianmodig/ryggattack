# Prediction continuation: safety hardening and verification

This supplements `client-prediction.md`; historical checkpoint results remain unchanged. No merge, deployment, production restart or public-room testing is authorized.

## Additional observed failures and corrections

A read-only source review identified concrete gaps in the initial checkpoint. Executed RED/GREEN regressions now cover:

- A maximum `u64` simulation clock overflowed prediction. Checked increment now falls back rather than panicking/wrapping.
- A newer publication number with a regressing simulation clock restored authority with a missing suffix. Simulation-clock regression is rejected independently of publication freshness.
- A collision/reversal correction replayed the old route's future steps. Bounded history now retains each actual travel state and compares the corrected tick's route; incompatible routes/large teleports clear the suffix and snap. Same-directed-path contact offsets remain correctable.
- Long stalls left remote interpolation state alive. Stall/horizon fallback now clears prediction, previous pose, correction, remote buffer and its jitter/cursor.
- Remote rail metadata could reference a grid edge absent from the authoritative map. Both restoration and remote interpolation now check the actual topology.
- Malformed optional movement data invalidated otherwise valid authority. Structural/range-invalid optional data now decodes as missing capability; invalid applied-input tick metadata is removed at the adapter callback. Mandatory gameplay/identity validation remains fail-closed.
- Missing actual local history steps silently shortened replay. A contiguous suffix through the local head is now required; otherwise restore authority and reset rather than inventing simulation steps.

The shared fixed-schedule contact fixture also characterizes a real rear-end push in both seat orders, shared speed expiry, and authority-only collision/effect application. It passes without changing those shared gameplay rules. This is native deterministic coverage, not browser collision evidence.

## Protocol and timing limitations

Protocol remains 4 with optional additive motion metadata. An old server gives authoritative-only presentation; an old client ignores the field. Held control `sequence/since_tick/axes` describes what the server selected at a completed simulation boundary; delivery `ack` is not a tick count. The client sends bounded held-input updates without waiting for an RTT. A local history entry is one actual 64-Hz shared travel step, never one transport packet.

Local tick labels are speculative correspondence anchored on received authority, not proof of synchronized cross-machine clocks or a claim that input takes effect on the same tick at both ends. Reconciliation restores server rail/speed/liveness immediately and replays only a complete recorded suffix beyond the corrected tick. Asymmetric latency and jitter must therefore be measured in the browser, including correction/convergence—not assumed correct from helper tests. No general combat rollback or prediction of collisions, pickups, wins, damage or projectiles exists. Anonymous rotating bearer reconnect remains unchanged and is not theft-resistant.

Remote interpolation targets a monotonic delayed server tick (125–250ms bounded adaptive delay), does not extrapolate indefinitely, and interpolates proven directed rail routes only. Local presentation interpolates adjacent actual steps; small same-path residuals decay exponentially, while incompatible routes and lifecycle discontinuities snap. These are pose targets for the replica renderer, never mutations of authoritative frame/HUD state.

## New tools and evidence boundaries

- `scripts/prediction-delay.mjs` is a test-only bounded ordered byte-delay fixture. Absolute due times overlap for successive chunks, preserving TCP order under deterministic jitter without imposing one delay per packet. Tests cover concurrent release, ordering/EOF, and hard queue bounds. No production deadlines are changed.
- `scripts/prediction-smoke.mjs` drives two separate Chromium processes, normal room menus and real keyboard input against an isolated loopback backend. It records opt-in bounded movement diagnostics, wire timing, source/backend/artifact hashes and original failures. Diagnostic positions are renderer pose targets, not independent GPU vertex measurements. Screenshots need visual review separately.
- `scripts/prediction-provenance.py` records an exact frozen source manifest and independently checks subsequent source/artifact bytes. A later commit does not relabel a dirty-source build as a fresh compile.

Current runtime is frozen in `verification/prediction-frozen-source.json`; the same bytes are committed in runtime head **9283f27**. The manifest deliberately preserves the original dirty-source/head relationship. Source and all nine generated/package artifact files were rechecked after browser execution. No runtime edit followed the build.

## Executed results

- Full frozen-source Rust suite: **252 passed**, including 205 library tests and 17 prediction integration tests; **230.690s** including compilation. Native binaries: **9.315s**. Clippy: **7.481s**, the same seven pre-existing warnings as fresh a48fbf5 (**6.584s** baseline). Initial stale-baseline shared-cache Clippy failure is retained; touching only the package source mtime forced the correct feature metadata to be checked.
- Selected **163 Node tests passed**, plus room UI, Pages redirect, portable container build contract, chart updater, strict Helm lint/chart rendering, TLS/banner/production-policy/isolated production smoke, all 16 raw-socket abuse checks and actual headless executable smoke. Detailed timings include the headless test's internal compile (**6.266s** total). A brief inadvertent native headless-smoke/Cargo overlap during the early WASM compile was not repeated; no compiler or optimizer ran during browser acceptance.
- Web release build: **413.573s**, including Cargo and wasm-bindgen; optional wasm-opt was intentionally absent for this local artifact. This is a tested local no-opt bundle, not a published release or the later CI optimizer output.
- Strict two-renderer attempt5: **185.817s**, separate Chromium processes, actual room menus/keyboard, zero delay and **90ms uplink / 150ms downlink ±25ms deterministic jitter**, 1MiB per-direction hard queue bound. The ordinary server rules/deadlines were unchanged. Saved-seat reload retained identity and resumed prediction after neutral release; natural match return cleared poses; a new rematch restored live carts/zero scores; later offline play used the same bundle. Independent offline-only replay also passed (**28.608s**) with zero WebSockets.
- At zero delay, host/guest had **10/21** local and **8/8** remote pose changes while the consumed authoritative publication was unchanged. Under added delay/jitter, these were **8/12** local and **8/5** remote changes. At common predicted/authoritative ticks, zero-delay comparisons matched **62/62** and **59/59** positions; delayed comparisons matched **63/64** and **66/67**. The two divergent samples were 0.2625 and 0.2165 rail/world units; later sampled authority-matching states were observed after 1450.7ms and 399.1ms respectively. Those sparse sampled intervals are **not** precise correction-settlement latency guarantees.
- Independent retained-data analysis found **185 host / 184 guest** delayed-case rendered pose observations before corresponding server processed-input confirmation: **34.7–353.7ms** and **112.3–353.2ms** respectively. Both traversed actual junctions and received authoritative speed buffs. This proves pre-confirmation travel presentation, not identical cross-machine steering application ticks.

## Limits and original failures retained

The successful quantitative viewport was **240×180**, pinned quality0. Mean render gaps were approximately 203ms under added delay: this is software-rendered correctness evidence, **not a claim of smooth 60FPS**. The tiny viewport's HUD obscures much of the playfield; a representative 480×360 capture separately showed normal rails/carts/HUD. Initial 480×360 gaps averaged 807.8/600.0ms and correctly activated the unchanged 500ms stall fallback. One subsequent short small-viewport run had no observable remote-between-publication sample and failed. Neither failed status was rewritten; the final strict replay used longer observation windows, retained the same movement assertions and product deadlines, and deferred its cumulative failure assertion until after lifecycle collection rather than deleting it.

Exact collision push/reversal geometry, speed expiry ordering, malformed capability data, history holes, clock overflow/regression and frame-rate-independent rail residuals are deterministic native/shared-pipeline evidence. The browser observes natural junction/speed/outcome/rematch behavior, but does **not** isolate every collision geometry, teleport or focus/background variant visually. Native graphical interaction remains deferred. No general combat rollback, device-bound reconnect claim, public-room acceptance or deployment is asserted.

Original producer records, executed harness/helper copies, inert runtime sources and a lossless compressed evidence inventory are retained in `docs/client-prediction-evidence/`. `python3 scripts/prediction-evidence.py` verifies the self-contained package; `python3 scripts/prediction-evidence.py checkout` independently compares the current runtime/artifact/helper bytes. `scripts/prediction-evidence.test.py` rejects missing, changed, extra and checksum-modified evidence. Text/decompressed text is privacy-scanned; representative screenshots were visually reviewed, not every archived screenshot. Final exact-head Forgejo CI and independent reviewer disposition remain separate gates.
