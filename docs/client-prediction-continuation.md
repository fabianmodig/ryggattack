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

Current runtime is frozen for the pending WASM/compiler and browser gate in `verification/prediction-frozen-source.json`. Do not infer full acceptance from this document or from a successful compile. Full rendered movement-before-confirmation, remote-between-snapshot, correction, lifecycle, offline and operational gates must each have actual linked results before the PR is ready.
