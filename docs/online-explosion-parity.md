# Online/offline explosion presentation parity

## Change

The previous online renderer used the correct unit-radius fireball mesh and
server scales, but painted every ball opaque orange. It omitted the offline
smoke, sparks, flash lights, ground scorch and flying prop parts.

Online now reuses the offline `EffectAssets` mesh/material palette and shared
auxiliary burst/animation code. A bounded optional `Body.explosion` descriptor
carries normalized source age, satellite ordinal and central-ball chain index.
The six original fire materials (cream/yellow → orange → dark translucent brown)
are selected from authoritative age. Source position/rotation/scale, creation and
removal remain server-owned; there is no local detonation, collision or damage
system. Primary and smaller chained blasts retain their existing geometry.

Cosmetic smoke, emissive sparks, the original brief point light, scorch and
existing prop-model fragments are animated locally. Only a newly observed
central ball starts a burst; satellite bodies never duplicate it. Material and
mesh handles are shared and not allocated per animation frame. Offline burst
construction/RNG order and wreckage math are preserved by extraction of the
original code, not replacement with a new effect.

The online app now installs the existing video-settings plugin and tags its
world camera. Effect counts and flash-light slots follow the same low/medium/high
budgets as offline. The explicit online menu launch also preserves valid `quality=0..6`
pins; otherwise entering a room discarded the initial query and switched to auto
quality. Invalid/absent overrides remain disabled.

## Review correction: matched pinned-quality progression

The initial montage below was insufficient for visual acceptance. A new
[paired quality0,960×480 comparison](evidence/explosion-parity-paired-q0/README.md)
now retains fully in-frame upper-right wall primaries through initial yellow,
main orange, dark transparent fade and removal, with identical full framing and
unscaled detail crops. Both original artifacts were reused without a rebuild or
runtime edits. Five actual video excerpts include normal-speed offline,
normal-speed human+bot online and two concurrent real online humans.

To expose stages on SwiftShader, the detailed paired fixture explicitly slows
the browser's monotonic clock to0.1×. Online additionally paces unchanged real
backend packets through a0.1× FIFO capture relay; the actual server, outbound
inputs and gameplay remain unchanged. This is presentation-only evidence, NOT
normal-latency or native-speed timing acceptance.144 delivered State packets
retained identical source/delivery hashes and increasing ticks. The unplayed
queued tail was discarded at explicit teardown, then ordinary Leave/readback and
clean browser/relay/backend shutdown completed. The new real-socket relay and
monotonic-clock tests passed2/2. No hardware-GPU benchmark is claimed.

The images now show comparable low-quality round/faceted geometry, cart-relative
scale, warm palette, transparency and disappearance rather than clipped arcs or
smoke alone. Gray haze, smaller nearby chain bursts, sparks and prop fragments
are visible in the extended sequences. Scenery/RNG, overlapping volleys and the
offline HUD/fog differ; this is the same representative effect progression, not
a pixel-identical seeded replay. A separate cream-color supplement explicitly
uses an earlier online primary, not a fabricated first frame for the final
primary. Both new harness failures and all older failures remain retained.
See the supplement for exact timestamps, provenance, privacy scope and verifier.

## Original comparison (retained history)

[Retained evidence and read-only verifier](evidence/explosion-parity-2026-10-28/README.md).

- Before: actual online browser images at main `20d0594`, showing opaque
  orange/red boundary discs. Its full replay FAILED the180-second budget and one
  clean browser-shutdown check; that status is preserved, not relabeled.
- Reference: unmodified offline WASM at the same main revision, quality6,
  640×480, actual keyboard firing. A real10-second video excerpt and contact
  sheet show a boundary burst turning yellow/orange and then brown/gray smoke.
  This is offline-browser rendering of the original effects, not a native-window
  capture. The native graphical path was not exercised.
- After, pinned0: two independent Chromium/SwiftShader processes, two actual WASM
  humans, a disposable loopback backend, ordinary room controls and held Space.
  Six distinct real screenshots, actual retained quality query, acknowledged
  inputs,1811 shared matching public frames, primary/wall/prop-chain/removals,
  privacy checks and clean Leave/browser/backend shutdown; exit0. The reviewed
  montage visibly contains pale cream, yellow/orange, dark fade, smoke and scorch
  instead of uniformly opaque red discs. Low quality intentionally has one
  fireball and fewer particles; do not compare its density with quality6.
- After, pinned6: the same two-browser/loopback setup at full reference quality,
  with shared spatial/temporal correlations/removals over321 matching public frames and
  clean shutdown; exit0. The separately labeled compositor supplement retains
  three genuine JPEG frames over60seconds, showing gray smoke, ground scorch,
  small sparks and a partly clipped orange boundary flare. It is NOT a pass of
  the six-screenshot gate. The attempted pinned6 screenshot gate timed out on
  its third screenshot and required one forced browser shutdown; retained intact.

Reference and candidate capture the same representative primary boundary-blast
and nearby prop-chain effect classes, not an identical seed/tick or synchronized
pixel replay. Different maps/camera framing and sparse software-rendered capture
prevent a pixel-equality or frame-accurate timing claim. Stronger per-stage
material/geometry agreement comes from the actual source→wire→replica ECS tests,
not from interpreting wire scales as measured GPU pixels.

## Verification

- `cargo test --locked`:220 passed,0 failed (191 library tests plus29 integration
  tests),4m20.627s. Four new regressions cover every fire-material stage for all
  three chain sizes without asset growth; quality budgets/dedup/match cleanup;
  old-server metadata omission; and malformed/bounded presentation metadata.
  Existing source-mesh world-vertex bounds and server/offline gameplay parity
  regressions still pass.
- Focused explosion filter:8/8 passed,1m21.648s including compilation.
- `cargo clippy --locked --all-targets`: exit0,7m53.409s;7 warnings, including3
  introduced renderer style warnings (two query-complexity warnings and one
  collapsible-if). This is not a warning-free claim.
- Full optimized WASM build:10m34.054s total, Rust6m41s, optimizer reported54→39MB.
  Builds used one Cargo job and one Binaryen worker, with canonical environment
  and shared target. No build overlapped rendered acceptance.
- Native candidate build0.322s; final server/probe/headless check0.392s. Earlier
  successful cold baseline native43m3.715s and baseline WASM15m16.519s are retained.
  Interrupted/fingerprint-mismatched attempts are not counted as successful
  builds; their original workspace logs remain untouched.
- After the WASM build, only `web/index.html` and `web/rooms.mjs` changed in the
  runtime. They were recopied with the build script's packaging operation.
  `checks/web-entry-repackage.log` proves unchanged WASM SHA and matching source/
  copied entry files. Both final pinned replays use these repackaged bytes and
  the same compiled Rust source; source snapshots/hashes are retained.
- Final Node gate160/160, room-UI including quality-preservation regression,
  Pages redirect, chart-version updater, real native untrusted-TLS rejection,
  server banner/policy, local production-mode contract, real socket suite,
  no-display headless gameplay, portable container-build contract, strict Helm
  lint and Helm deployment-contract tests all exited0. Production-mode tests
  spawned local disposable binaries; they did not touch the live service.
- Package verifier:49 manifested files,3544 full-stream shared frames recomputed
  (2132 at the original report checkpoints),232 checkout source/artifact paths
  compared; self-contained and checkout modes exit0. Five negative/positive
  verifier trials passed, including rejection of semantically changed frames and
  report checkpoints even after their fixture manifests were resigned. Later
  capture/Leave frames are checked separately from the earlier report checkpoint.
  The verifier hashes retained source/images/logs and rejects unexpected members.

## Performance and deliberate differences

Pinned0 gameplay rAF means were933.1575ms and932.88ms (120 samples/client), with
p95 intervals1566.6/1599.9ms. These are event-loop observations on two CPU software
renderers, not GPU timings or a hardware-browser benchmark. There is no reliable
before/after FPS benchmark, so no numeric speedup/no-regression claim. Both
candidate runs' authoritative receive clocks stayed approximately64Hz while
rendering was slow; exact calculations are in `performance.json`.

The pinned6 rAF sampler was read before callbacks accumulated and has0 samples;
its null metrics are unavailable. Three compositor frames/minute are a capture
limitation, not a measured game frame rate. The full-quality screenshot timeout
and earlier automatic-resume startup failure are disclosed, not hidden by the
later successes. The corrected fixture waits for each real lobby presentation
before starting the next isolated renderer; no server timeout was changed.

The additional work is the missing offline art: transparent fire/smoke, bounded
sparks and at most the source-quality flash-light count. Shared assets, low
budgets and no per-frame asset churn bound it; a representative hardware-GPU
performance check remains advisable.

- Authoritative fireball age/size/removal are sampled at network/render cadence,
  so an overloaded browser can skip early colors entirely. The sparse full-quality
  capture cannot verify every stage or fine flash timing visually.
- Auxiliary particle positions use a separate deterministic cosmetic RNG keyed
  by network ID, not the server/offline gameplay RNG. Counts/materials/motion are
  shared, but individual spark/debris trajectories are not pixel-identical.
- Smoke/sparks/scorch begin on first observation; point-light flashes are
  suppressed when a central ball is first seen beyond40% progress. This avoids
  replaying a stale flash on late admission. Auxiliary age is not network-synced.
- Prop fragments are cosmetic, triggered only when an existing replicated prop
  disappears near an authoritative central blast. A missed entire blast can omit
  the fragments; no local destruction or damage is inferred.
- Older protocol4 servers without the optional metadata still render source-size
  balls and remove them normally, but use the initial source material and lack
  auxiliary reconstruction. Full parity requires the paired new backend. Older
  clients ignore the optional descriptor; gameplay protocol/timing are unchanged.
- No new bloom style was invented. The original unlit/alpha fire palette,
  emissive sparks and non-shadow-casting point lights are reused. Actual flash
  illumination and every particle trajectory are not separately GPU-measured.

No merge, deployment, production restart or public-room test is authorized or
performed. Review the images and limitations before deciding whether additional
hardware-GPU acceptance is needed.
