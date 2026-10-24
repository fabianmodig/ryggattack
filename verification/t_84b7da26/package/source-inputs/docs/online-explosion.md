# Local online explosion artifact gate

## Scope and current status

`scripts/online-explosion-smoke.mjs` is an actual-artifact harness, not itself
recorded gameplay evidence. Authoring checks cover its syntax, conservative
wire-evidence classifier, and shared shutdown/finalization helpers using mocked
child/UI boundaries. Do not claim a rendered or mesh-bounds pass from those checks.

The intended gate is two independently launched Chromium processes, each with
one real rendered WASM human, against one disposable native server bound to
`127.0.0.1:0`. It serves the parent's real `dist/web` bytes at an ephemeral
loopback HTTP origin. Quality is `0`, viewport `640x480`, software SwiftShader.
The room uses Forest, two distinct humans (`BlastA`, `BlastB`), **zero bots**,
and first-to-seven. The harness does not require a match winner: legal shots,
impacts, destruction and visuals are the acceptance target.

No public server, production deployment, protocol/state injection, WebSocket
replacement, fake application globals, forced winner, synthetic gameplay, or
changed server deadline is permitted. Native graphics remain deferred. This
local command grants no authorization for a public-room test.

## Parent artifact conditions and exact invocation

The parent must first finish the Rust worker's canonical explosion-mesh fix and
its **actual mesh construction/bounds tests**, then finish the native backend and
**full** release WASM build (including matching wasm-bindgen, optimizer, and all
web entry modules). No Cargo/build/browser/server command is run by the Node
unit checks below. Do not overlap the artifact run with a build, another heavy
renderer, or a source edit.

Parent-owned build/test commands, only after exclusive Cargo ownership returns:

```sh
cargo test --locked online::tests:: -- --nocapture
cargo build --locked --bin ryggattack-server
bash scripts/build-web.sh
```

The test filter must actually include the newly added kind-4 mesh/bounds tests;
a zero-selected-test invocation is **not** evidence. Retain their names, counts
and output from the Rust worker. Tests must measure real `Mesh` vertex bounds
for primary and smaller chain transforms and protect non-explosion mesh choices;
a JS scalar calculation is not a substitute. If the Rust worker places the
shared canonical-mesh tests elsewhere, run those too using its exact test names.

After the parent freezes those artifacts:

```sh
cd /home/hermes/src/ryggattack-online-explosions
RYGGATTACK_EXPLOSION_ARTIFACT_READY=1 \
PLAYWRIGHT_NODE_MODULES=/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json \
RYGGATTACK_NATIVE_BIN_DIR=/home/hermes/src/ryggattack-online-explosions/target/debug \
node scripts/online-explosion-smoke.mjs --local
```

`RYGGATTACK_NATIVE_BIN_DIR` may point to another parent-built native directory.
No server URL/origin can be supplied. `--local` and the explicit artifact-ready
acknowledgment are required. The harness builds nothing. Missing binaries,
missing/generated-invalid WASM, stale copied web modules, wrong served
hashes/MIME, symlinks, or Rust sources newer than the backend/WASM fail preflight.
The timestamp test is a stale-build guard, **not cryptographic compilation
provenance**: the parent must bind successful build logs to the recorded source
and generated artifact hashes. Do not merely touch old files to satisfy it.

Lightweight authoring regressions (safe before the build freeze):

```sh
node --check scripts/online-explosion-smoke.mjs
node --check scripts/online-explosion-evidence.mjs
node --check scripts/online-explosion-cleanup.mjs
node --test scripts/online-explosion*.test.mjs
```

## What is asserted

- One admission/socket per client, protocol **3**, actual browser Origin, distinct
  process IDs, participant IDs, and seats in the same owned room. Both clients
  observe authoritative human-name/seat mappings and two active carts.
- Ordinary HTML Create/Join/Configure/Ready controls, authoritative countdown and
  Running. No command injection from `evaluate`; evaluations only read public
  presentation/DOM (canvas focus uses the real DOM focus mechanism).
- Space is **held down on both pages** with canvas focus and the entry form
  closed, until each produces at least three firing Inputs while authoritative
  ticks advance. The full legal input history is saved. At least one firing
  sequence per participant is acknowledged by the server. Steering stays zero.
- Both observers receive matching complete public State snapshots at every
  shared tick, including cart poses, bodies/IDs/transforms, names, scores and
  acknowledgment arrays. Each match is analyzed separately so a new simulation
  cannot alias old IDs. A union of disjoint observations cannot pass.
- Positive finite isotropic kind-4 scales and stable explosion centres/IDs. The
  shared-frame classifier requires a large primary (sampled maximum >= 1.4),
  a nearby disappeared projectile, and a boundary primary centred on the arena
  wall at absolute X/Z 9. It also requires a disappeared prop within primary
  reach and a **different, later** smaller kind-4 ID at that prop's X/Z centre
  (sampled maximum 0.65..0.9). Both primary and chain must subsequently disappear
  in one contiguous unfinished-round segment (both ticks advance by four).
  Later gaps/round clears preserve already completed historical evidence but
  cannot extend unfinished tracks or connect a primary to a later chain.
  A round clear, sampling gap, no shots,
  unrelated prop, empty observation, or tiny-only wire bodies cannot pass.
- These are conservative **spatial/temporal wire correlations**, not causal tags:
  protocol 3 does not label explosion generations or collision reasons. Retained
  snapshots allow review of the correlation. Actual Rust simulation/mesh tests
  supply the stronger causal/geometric oracle. If this fixture does not produce
  the required chain/wall sequence naturally, the run fails; do not weaken the
  requirement or patch gameplay to manufacture it.

## Screenshot sequence and budgets

The harness requests three **distinct actual PNGs per observer**, sequentially.
Before **each** capture it reestablishes genuine Running if an earlier software
capture spanned a naturally completed match, using normal Ready controls only.
It then requires fresh wire kind-4 bodies with scale >= 0.5, checks the actual
public HUD's Running/own-seat state, distinct rendered names, and visible canvas
geometry. Both participants keep holding Space during capture.

Each screenshot has a **60-second capture-only timeout**. This is not a server
result/reconnect deadline. The entire gameplay/capture opportunity budget is
**180 wall-clock seconds from first observed Running**, including screenshot
latency, with at most three natural match starts. Captures cannot extend it. A
slow capture can exhaust that budget, and that attempt must fail. Startup,
normal page actions, waits and graceful/forced cleanup also have bounded waits.
No forced endgame or natural endmatch requirement is used just to inspect scale.

PNG metadata retains begin/end receipt times, match IDs, authoritative ticks,
pre-capture explosion IDs, read-only public HUD phase/tick/seat/name information,
and image hashes. A screenshot can span a phase transition or miss a transient
blast. The metadata **does not synchronize a particular wire tick to pixels**.
The sequence may span separate natural blasts; it is not a guaranteed high-FPS
movie of one ID. Do not claim otherwise.

**Required manual image review before acceptance:**

1. Inspect all six PNGs and their timing/state metadata. At least two images per
   observer must visibly show explosions in the real arena, not a blank canvas,
   loading screen, form, or menu. Compare their size to visible carts/rails.
2. Confirm the blasts are visibly enlarged rather than the old pinprick balls,
   and that sequence images show actual changing blast appearances. If captures
   missed the blasts, repeat a new immutable attempt; wire-only success is not
   a visual pass. A phase-crossing image is not evidence of its earlier phase.
3. Retain the actual CPU mesh-bounds test output with this artifact revision.
   The defect changes the kind-4 **mesh**, not its already-correct wire transform:
   **the old broken build can pass the wire classifier**. The authoritative
   canonical sphere uses radius 1 / ico(2), whereas the defective online reuse was
   radius 0.16. Rendered images are qualitative visual evidence; measured Mesh
   bounds plus exact replicated transforms are the quantitative oracle.
4. Record the reviewed PNG filenames/hashes, source/artifact hash identity,
   bounds-test result and reviewer conclusion in a separate review record. Do
   not edit the original attempt to turn a failure into a pass.

CPU-only mesh tests can establish geometry without a native graphics run, but
cannot establish successful WASM drawing. Conversely, screenshots/wire parity
cannot quantitatively prove vertex bounds. If either side is unavailable,
report that limitation and keep the combined artifact gate incomplete.

## Evidence, privacy, and cleanup

Each invocation creates a unique, never-reused
`verification/online-explosion/<timestamp>-<random>/` directory:

- `results.json`: explicit status/errors, build input SHA-256/byte manifest,
  source revision/worktree context, shared-frame proof and screenshot metadata;
- `source-inputs/`: exact consumed Rust/web/script/Cargo source bytes, including
  a dirty worker patch, checked against the initial hashes;
- `inputs.json`, `states.json`, `lobbies.json`: whitelisted public observations
  from each observer, including every legal Input and control-command kind;
- `BlastA-{1,2,3}.png`, `BlastB-{1,2,3}.png` when captures succeed;
- `sha256.json`: final artifact checksums. Failed/partial attempts remain
  separately identified; missing images are never synthesized.

All source, helper, generated artifact and backend hashes are rechecked after
cleanup. Frozen inputs must remain unchanged through the run. No output path
is reused and no prior failure is overwritten by a retry. Generated binaries
are hashed, not duplicated; retain the parent build artifacts/logs separately.

The harness reuses the existing `canonical`/`redact`, copied-entry preflight,
private-screenshot and evidence-publication helpers **without modifying them**.
Raw Welcome messages are never retained, nor are private storage, cookies,
headers, browser connection endpoints, or share URLs. Only explicit public
identity/Lobby/State/input fields are recorded; server URLs are omitted from
free-text diagnostics. Private values discovered from protocol messages are
kept only in memory for redaction/negative scans. Screenshot DOM text is checked
before capture; final JSON/source/image artifacts are scanned for known private
values. Privacy failure withholds/purges that attempt's artifacts, retaining
only a failure marker. Binary scans are not OCR.

Cleanup is part of the gate: both pages use their ordinary Leave buttons; the
actual Leave packets, socket closures, cleared public room/HUD readbacks and
empty HUD roster/outcome are required. The first leave is also checked against
the remaining observer's authoritative roster. Pages close, both owned browser
processes must exit, the owned backend is terminated and awaited, and the HTTP
listener closes. Browser/backend exit is accepted only after the harness requests
termination, with code zero and no terminating signal. An already/unexpectedly
dead process (even code zero), nonzero exit, signal exit or forced kill fails.
Backend SIGTERM requests must therefore finish gracefully with code zero, not
terminate by signal. Missing admitted-client Leave/HUD/socket flags or the first
peer's roster readback fail finalization even if a page is already closed.
SIGINT/SIGTERM during cleanup still permits bounded cleanup but forces failed
status and nonzero exit. No unrelated process is targeted. Even a failed run
attempts all cleanup paths.

Browser shutdown has a separate **20-second total close watchdog**, including
the existing at-most-five-second connected-client disconnect wait. The backend
retains its five-second close watchdog; post-close exit observation remains two
seconds. These are cleanup-only bounds: gameplay stays 180 seconds and each
capture stays 60 seconds. The browser-specific helper is shared with virtual-clock
regressions; forced kills and abnormal/unexpected exits remain failures.

Run30 local diagnosis retained the old algorithm with phase timestamps: the
connected-client disconnect resolved in 1ms, but BlastA was killed at the old
five-second owner-close deadline. An owner-first control failed the same way,
so changing disconnect order was not a sufficient repair. With the original
order and a 20-second shutdown-only watchdog, the actual owned process exited
cleanly 5924ms after the owner close request (BlastB: 2668ms). This establishes
a premature watchdog, not the internal Chromium reason for its teardown latency.
Diagnostic copies, traces and immutable attempts are under
`verification/explosion-repair-run30-*` and `verification/online-explosion/`.
These repair checks do not approve the original acceptance or replace its
independent image review and rerun.

Review-fix RED/GREEN logs, exact executed source snapshots, timing/count/hash
records and diffs are retained in `verification/explosion-harness-review-fix/`.
Those are Node authoring evidence only, not an executed browser/artifact gate.

Exit zero means **`captured-review-required`**, not a measured/render-reviewed
scale pass. Nonzero means failed preflight/gameplay/privacy/cleanup. Never infer
success from partial PNGs or Node unit regressions; complete the image review
and the real Rust mesh-bounds gate against these exact hashes first.
