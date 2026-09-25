# Ryggattack browser performance report

Baseline: `main` @ `61fd92a`, the release build GitHub Pages serves
(<https://fabianmodig.github.io/ryggattack/>, deployed 2026-09-12, built with
thin LTO and `wasm-opt -Oz`). The wasm is 39.3 MB, or 11.8 MB gzipped.

**All file:line references point at `main` @ `61fd92a`.** The
`perf/web-video-settings` branch, where this report is committed, has already
moved some of those lines.

## 1. What the game is

- **Engine:** Bevy 0.19.1 (Rust), compiled to `wasm32-unknown-unknown` via
  wasm-bindgen. Rendering is Bevy's PBR renderer through wgpu on **WebGL2**.
  It is not a Canvas2D or JS engine, and there is no DOM rendering apart from
  one `<canvas>` in `web/index.html`.
- **Main loop:** the winit web event loop, driven by `requestAnimationFrame`.
  `PresentMode::AutoVsync` is set in `src/main.rs:38`. Everything runs on one
  thread, because wasm has no Bevy multithreading: ECS systems, render-world
  extraction and WebGL command encoding all share the browser main thread.
- **Gameplay systems:** a single chained `Update` set, in `src/main.rs:81-105`.
- **Scene:** built once in `scene::setup` (`src/scene.rs:29-144`). The pieces
  are one directional light with shadows, distance fog, a ground slab, the
  track, about 500 forest props made of 2-4 mesh parts each, 4 carts with
  about 22 mesh parts each, and the HUD.
- **WebGL2 fallbacks** that Bevy logged at startup: "GPU clustering isn't
  supported … falling back to CPU clustering", "GPU preprocessing is not
  supported … Falling back to CPU preprocessing", and "SSAO plugin not loaded".
  Light clustering and mesh preprocessing therefore run on the main thread
  every frame.

## 2. How it was measured

| | |
|---|---|
| Host | Proxmox LXC, 4 vCPU Intel Core i5-6360U @ 2.0 GHz, 5 GiB RAM, **no GPU** |
| Browser | Playwright Chromium 1243, headless, WebGL2 via ANGLE + **SwiftShader** (CPU rasteriser) |
| Scene | Main menu → lobby (WASD takes P1, P2-P4 are bots) → START ROUND. "idle" = round running, P1 not firing (bots still shoot). "fight" = P1 holds fire the whole window. |
| Metrics | rAF interval (FPS, median, p95); CDP `Performance.getMetrics` TaskDuration/ScriptDuration for renderer main-thread busy time per frame; every WebGL2 call wrapped to count draws, GL calls, triangles and buffer uploads; V8 CPU profile (`Profiler.*`); Chrome trace (`browser.startTracing`) for per-thread busy time. |
| CPU throttling | `Emulation.setCPUThrottlingRate` at 1x, 4x and 6x |
| Scripts | `scripts/perf/ingame.mjs` (FPS, GL counts, main-thread time, `--throttle`, `--dpr`, `--ablate shadow\|corners`) and `scripts/perf/anatomy.mjs` (per-pass draw breakdown). Serve a `dist/web` directory and run e.g. `node scripts/perf/ingame.mjs dist/web base --width 320 --height 180 --throttle 1,4`; set `PLAYWRIGHT_NODE_MODULES` if Playwright is not installed locally. |

Caveats, so the numbers are read correctly:

- **There is no hardware GPU, so absolute FPS is meaningless.** With
  SwiftShader, rasterisation runs on the CPU in Chrome's GPU process. The
  trace shows `GPU Process/CrGpuMain` busy for 6.8 s of an 8 s window, while
  the renderer main thread was 96-97 % idle. The frame rate below is therefore
  "how fast a CPU can rasterise this scene", which exaggerates fill-rate and
  vertex costs. Use it for relative comparisons only (baseline vs ablation vs
  fix, same viewport).
- **The metric that carries over to real machines** is renderer main-thread
  busy time per frame (wasm, JS glue, and WebGL command encoding). On a real
  GPU this work bounds the frame rate.
- At 1280×720, fewer than 2 frames completed in a 6-8 s window. That gives
  "0.1-0.2 fps", which cannot be measured further, so the in-game tables use
  a **320×180** viewport. The GL counts do not depend on resolution.
- `RailMap::random()` (`src/main.rs:25`) picks a new map and forest on every
  page load, so scene content varies a little between runs. Triangles per
  frame stayed at about 300k in every run. Each window holds only 6-15
  frames, so single-run differences under about 15 % are noise.
- The deployed wasm has no name section, so the CPU profile cannot attribute
  wasm time to Bevy or game functions. Building locally with names was not
  possible: `wasm-bindgen` on the 74 MB `web-dev` module needs more than
  3.3 GB and was OOM-killed on this 4-5 GB host.

## 3. Baseline numbers

### 3.1 Frame rate and main-thread cost (320×180, DPR 1)

| Window | FPS | median frame | p95 frame | main-thread busy / frame | script / frame | draws / frame | WebGL calls / frame | triangles / frame |
|---|---|---|---|---|---|---|---|---|
| menu (3D scene behind menu) | 0.90 | 1233 ms | 1367 ms | 21.8 ms | 20.9 ms | 538 | 12 336 | 296 912 |
| **idle, 1x CPU** | 0.71 | 1750 ms | 3850 ms | **23.5 ms** | 22.5 ms | 564 | 13 306 | 301 036 |
| **fight, 1x CPU** | 0.62 | 1833 ms | 3283 ms | **46.5 ms** | 45.1 ms | 599 | 14 516 | 302 902 |
| idle, 4x CPU | 0.37 | 2167 ms | 4933 ms | **92.4 ms** | 85.0 ms | 565 | 13 336 | 275 773 |
| fight, 4x CPU | 0.45 | 2217 ms | 4116 ms | **119.4 ms** | 116.1 ms | 560 | 13 305 | 271 882 |
| idle, 6x CPU (1280×720 run) | <0.2 | n/a | n/a | **156.9 ms** | 116.5 ms | n/a | n/a | n/a |
| fight, 6x CPU (1280×720 run) | <0.2 | n/a | n/a | 95.4 ms | 70.5 ms | n/a | n/a | n/a |

What this means for a real GPU machine:

- The main thread alone costs **about 22-24 ms per frame when idle, and up to
  about 46 ms in a fight**, on a 2015 dual-core laptop CPU. That caps the game
  at roughly 40 fps idle and roughly 20 fps in a fight before the GPU does any
  work.
- On a 4x slower CPU (a low-end Chromebook or phone) the cost is **92-119 ms
  per frame, so 8-11 fps**. This matches "laggy in the browser" and is the
  number to drive down.
- In later runs the fight cost varied more (19-22 ms, see 3.3). The fight
  overhead depends on how many blasts happen to be alive, so treat
  "fight ≈ 1-2x idle" as the range.

Other measurements:

- **Memory and GC:** JS heap 8.1-8.9 MB, flat for the whole session. GC took
  34 ms of self time over 12 s. **No GC churn.** Rust allocations live in wasm
  linear memory and are not GC'd.
- **Uploads:** 15 `bufferSubData` calls, about 32 KB per frame. Fine.
- **Load:** 1.3 s from localhost. Over the network it is 11.8 MB gzipped
  (Pages sends gzip with `max-age=600`). This is not a frame-rate problem.

### 3.2 Where the draws go (one fight frame, `anatomy.mjs`)

| Pass | Draw calls | Instances | Single-instance draws | Non-indexed draws (`drawArrays`) | Vertices |
|---|---|---|---|---|---|
| Shadow map (directional light, depth only) | **272** | 2 156 | 158 | 123 | **656 286** |
| Main opaque pass | **233** | 1 256 | 157 | 123 | 364 698 |
| Transparent pass (smoke, fireballs, scorches) | 50 | 87 | 35 | 0 | 44 964 |
| UI | 1 | 1 | 1 | 0 | 420 |
| Tonemapping and final blit to canvas | 2 | 2 | 2 | 2 | 6 |

- The **shadow pass has more draws and 1.8x the vertices of the main pass.**
  It renders every caster inside the 40-unit cascade, including props outside
  the camera frustum.
- The **123 `drawArrays` calls per pass are the track corners.** Each corner
  gets its own `meshes.add(...)`, so none can be instanced. With the shadow
  pass that makes 246 draws per frame, about 45 % of all draws.
- The top WebGL calls per frame are `texParameteri` 2590, `bindSampler` 1305,
  `activeTexture`/`bindTexture` 1296, `bindBuffer` 804, and
  `vertexAttribPointer`/`enableVertexAttribArray`/`vertexAttribDivisor` about
  743 each. That is roughly 23 GL calls per draw. Each call crosses
  wasm → JS → the Chrome command buffer on the main thread, so **main-thread
  cost scales with draw count.**

### 3.3 Ablations on the same build

These were done by filtering WebGL draw calls in the page, with no rebuild,
at 320×180 and 1x CPU.

| Variant | idle median frame | fight median frame | vs baseline |
|---|---|---|---|
| Baseline | 1750 ms | 1833 ms | n/a |
| Shadow-map draws skipped | 1167 ms | 1217 ms | **-33 % / -34 %** |
| Track-corner draws skipped | 1117 ms | 1050 ms | **-36 % / -43 %** |

These are SwiftShader numbers, so they overstate vertex and fill-rate costs,
but the direction holds on real hardware. With `--dpr 2` the canvas backing
store stayed at CSS size (320×180 at both DPR 1 and DPR 2). **High-DPI
over-rendering was not observed**, and the render resolution is the window
size.

## 4. Ranked bottlenecks

Impact is estimated from the measurements above plus static analysis.
**No visual change** means an engineering fix with identical output; these
belong to the "visual-neutral optimisations" task. **Visual tradeoff** items
are candidates for the user **video settings** menu.

### #1 Draw-call count: every mesh part is its own entity, and track corners cannot be instanced
**Impact: high.** Main-thread cost is about 23 WebGL calls per draw, at
560-600 draws per frame. Removing only the corners cut SwiftShader frame time
by 36-43 %.

- **Track corners:** `src/tracks/render.rs:81-85`. Each corner piece calls
  `meshes.add(track_corner_mesh(...))`, which gives a unique mesh per corner
  (3 per junction turn) and 123 non-instanceable draws per pass.
  `track_corner_mesh` (`render.rs:106-157`) also emits non-indexed geometry
  with `TRACK_CORNER_STEPS = 24` (`render.rs:17`).
  **Fix (no visual change):** at startup, merge all static track geometry
  (corner beds, rails, straight beds, rails, sleepers from
  `render.rs:159-217`) into **one mesh per material** (3 meshes: bed, sleeper,
  rail). Nothing in the track ever moves or is destroyed. That turns about 250
  draws into 6 (3 shadow + 3 main).
- **Forest:** `src/scenery.rs:116,120` define 64 arena props and 440 outer
  props. `spawn_prop` (`scenery.rs:296-318`) and `parts` (`scenery.rs:322-428`)
  spawn 2-4 child mesh entities per prop, about 1 500 mesh entities in total.
  Bevy instances identical mesh+material pairs, but on WebGL2 the batches are
  split by the uniform-buffer limit. The histogram shows 77 draws of 17-64
  instances plus 158 single-instance draws.
  **Fix (no visual change):** some props can never be reached by a blast.
  Missiles detonate inside the walls (`combat.rs:176-181`); the blast radius
  is 2.0 plus a 1.3 chain (`explosions.rs:15,21`) and props have a hit radius.
  Bake those props into merged static meshes per material, in spatial
  patches, so frustum culling still works. Keep the reachable ones as
  entities. The `perf/web-video-settings` branch already has a start on this
  in `src/batch.rs` and the `scenery.rs` backdrop baking.
- **Carts:** `src/players/visuals.rs:99-190` spawn about 22 child meshes per
  cart. Materials are per player (`visuals.rs:72-79`), so parts cannot be
  instanced across players, which costs about 60-80 draws per pass.
  **Fix (no visual change):** merge the rigid cart and rider parts of each
  player into one mesh per material at spawn. They are all static relative to
  the cart root.
- **Missiles:** `src/combat.rs:117-155` spawn 5 entities per missile (body,
  nose, 2 fins, exhaust). They are instanceable, but they add ECS, transform
  propagation and extraction work. **Fix (no visual change):** one merged
  missile mesh per owner colour, plus the exhaust.

### #2 The shadow pass re-renders the scene
**Impact: high.** The shadow pass has 272 draws and 656k vertices, more than
the main pass. Skipping it cut SwiftShader frame time by about 33 %.

- `src/scene.rs:26-27` set a 40-unit cascade with a 1024² shadow map;
  `scene.rs:50-67` set up the directional light with `shadow_maps_enabled` and
  one cascade.
- Casters with no visible shadow still cast. The ground slabs
  (`scene.rs:83-101`, including the 100×100 forest floor) and the track bed,
  rails and sleepers, which sit 0.02-0.16 above the ground
  (`render.rs:178-216`), are all rendered into the shadow map.
- **Fix (no visual change):** add `NotShadowCaster` to the ground slabs, the
  track pieces and the walls' undersides. Once #1 bakes the outer forest, give
  the baked far patches their own caster flag. Also check whether the cascade
  can hug the view frustum: `maximum_distance` 40 already matches the frame,
  and `first_cascade_far_bound` and overlap are at defaults.
- **Visual tradeoff (video setting "Shadows": Off / Low 512 / Medium 1024 /
  High 2048):** turning shadows off removes the whole pass. Lower resolution
  saves fill-rate. The `perf/web-video-settings` branch already lets the
  settings own the shadow map size.

### #3 Triangle count from default primitive resolutions
**Impact: medium-high on GPU and vertex work.** The scene is about 300k
triangles per frame across the passes. By entity count, the forest's
cylinders and cones are most of it. This split was estimated from the code,
not measured per mesh.

- `Cylinder::new` and `Cone::new` use Bevy's default `resolution: 32`
  (`bevy_mesh-0.19.1/src/primitives/dim3/cylinder.rs:46`, `cone.rs:38`). They
  are used for every trunk, pine tier, fern frond and stump
  (`src/scenery.rs:162-163`). A pine is 1 cylinder plus 3 cones, about 320
  triangles; a fern is 4 cones, about 256.
- `Sphere::new(r)` meshes default to **ico subdivision 5**
  (`sphere.rs:46`, 362 vertices / 720 triangles). That applies to rider
  heads, hair, hands and eyes (`src/players/visuals.rs:35,36,40,42`) and to
  the missile exhaust (`src/combat.rs:54`). Scenery and effect spheres
  already use `ico(2)` (`scenery.rs:164-169`, `explosions.rs:92-97`).
- **Fix (no visual change at the game's camera distance, about 23 units;
  confirm by screenshot diff):** use resolution 8-12 for trunks, fronds and
  stumps and 12-16 for pine tiers. Use `ico(1)` for eyes and hands and
  `ico(2)` for heads, hair and exhaust. A 0.07-radius trunk at 32 segments is
  sub-pixel detail. This should give about 3x fewer triangles.
- The 24-step track corners (`render.rs:17`) can drop to 12 with no visible
  difference at this zoom.

### #4 Effect entity churn and material swapping in fights
**Impact: medium on CPU, growing with the number of players firing.**
Main-thread cost went from 23.5 to 46.5 ms per frame, idle to fight, in the
baseline run. The transparent pass has about 50 draws in a fight.

- **Missile smoke trail:** a new smoke-sphere entity every
  `TRAIL_INTERVAL = 0.03 s` for each missile (`combat.rs:24`, `166-174`),
  each living 0.45 s. That is about 15 live puffs per missile, and several
  hundred alpha-blended, depth-sorted spheres when 4 carts spray.
- **Each missile blast** (`explosions.rs:301-399`) spawns 3 fireballs, 14
  sparks, 4 smoke puffs, a scorch and a light: about 23 entities. Each chain
  blast adds about 14 more, and `wreck` (`explosions.rs:263-295`) adds 2-4
  debris entities for every destroyed prop.
- **Every frame, every fireball and smoke puff replaces its material handle**
  (`explosions.rs:449`, `475`). That forces material re-extraction and
  re-batching even when the stage has not changed.
- **Fixes (no visual change):**
  - Only write `material.0` when `stage(...)` actually changes.
  - Pool effect entities (hide and reuse) instead of `spawn`/`despawn`, which
    avoids archetype moves and command-buffer churn.
- **Visual tradeoff (video setting "Effects": Low / Medium / High):** trail
  interval (0.03 s → 0.06 / 0.09 s), spark count (`14.0 * size`,
  `explosions.rs:351`), smoke puffs per blast (4, `explosions.rs:371`), debris
  per wrecked prop, chain-blast depth (`CHAIN_BLAST_RADII`,
  `explosions.rs:21`), and scorch lifetime (`SCORCH_SECONDS`,
  `explosions.rs:30`). The branch already wires an effects budget into
  `explosions.rs` and `combat.rs`.

### #5 4x MSAA by default
**Impact: medium on fill-rate and bandwidth-bound integrated or mobile GPUs.
Not measurable on SwiftShader at 320×180.**

- The camera (`src/scene.rs:37-48`) has no `Msaa` component, so Bevy's
  default `Msaa::Sample4` applies (`bevy_render-0.19.1/src/view/mod.rs:243-244`).
  The main colour and depth targets are 4x multisampled and resolved every
  frame. The canvas itself is not multisampled (`gl.SAMPLES = 0`), which is
  as expected: Bevy renders offscreen and blits.
- **Visual tradeoff (video setting "Anti-aliasing": Off / 4x MSAA):** WebGL2
  offers no 2x through wgpu. FXAA would be a cheap middle option.

### #6 Render resolution follows the full window
**Impact: medium on high-resolution screens (fill-rate), tradeoff only.**

- `fit_canvas_to_parent: true` (`src/main.rs:42`), with `#game canvas` at
  100 %×100 % (`web/index.html`). A 2560×1440 browser window renders 3.7M
  pixels, times 4 for MSAA, plus a shadow map, fog and tonemapping. DPR did
  not multiply the backing store in the measurement (§3.3).
- **Visual tradeoff (video setting "Render scale": 50 / 75 / 100 %):** set
  `WindowResolution::with_scale_factor_override`, or render to a smaller
  target and upscale.

### #7 HUD text rebuilt every frame
**Impact: low-medium on CPU.**

- `update_hud` (`src/ui/mod.rs:73-98`) allocates a `Vec`, a sorted copy,
  several `String`s and a `format!` every frame, and assigns `hud.0`
  unconditionally. That marks `Text` as changed, so Bevy UI re-shapes and
  re-lays out the text every frame.
- **Fix (no visual change):** cache the last `(remaining seconds, scores,
  finished)` and write only when it changes, which is about once a second.

### #8 Linear scans of all scenery per missile and per blast
**Impact: low** (about 25 missiles × 504 props ≈ 12k distance checks per
frame).

- `strike_scenery` (`src/combat.rs:197-213`) tests every missile against
  every prop, including the 440 outer props a missile inside the walls can
  never reach. `run_detonations` (`src/explosions.rs:232-258`) scans all
  scenery for every detonation. Each frame also allocates a `HashSet` in
  `explosions.rs:209`, and a `Vec` plus `HashSet` in `combat.rs:221-232`.
- **Fix (no visual change):** give reachable props a marker component, or use
  a coarse grid, and query only those. Reuse the scratch collections through
  `Local<>`.

### #9 Minor and already mitigated
- **Point lights:** capped at 3 flashes with a range of 6 already
  (`explosions.rs:33-38`). Clustering runs on the CPU on WebGL2. That is
  fine at this count.
- **Distance fog:** cheap per fragment (`scene.rs:40-47`).
- **Player-to-player collision:** 4 players, so O(n²) is trivial.
- **Unused 1.2 MB PNGs:** `assets/players/*.png` (1254² RGBA) are not
  referenced from `src/`. They cost no runtime, but they are shipped. Fix:
  remove them or keep them out of `dist`.

### Checked and not a problem
- **rAF usage:** there are no loops outside rAF, and winit drives frames from
  rAF.
- **DOM layout thrash:** one canvas; the status div is removed after load.
- **JS GC churn:** heap flat at about 8 MB, with 34 ms of GC in 12 s.
- **Per-frame buffer uploads:** 32 KB.
- **Texture sizes:** no textures are loaded at runtime; every material is a
  flat colour.
- **devicePixelRatio over-rendering:** not observed.

## 5. Summary table

| Rank | Bottleneck | Where | Est. impact | No visual change | Tradeoff (video setting) |
|---|---|---|---|---|---|
| 1 | Too many draws: per-part entities, unique track-corner meshes | `tracks/render.rs:81-85,159-217`; `scenery.rs:116-120,296-428`; `players/visuals.rs:99-190`; `combat.rs:117-155` | High: 560-600 draws → ~150-200; main thread ~-40-60 % | **Yes**: merge static geometry per material; bake unreachable forest | Forest density (fewer outer props) |
| 2 | Shadow pass redraws the scene | `scene.rs:26-27,50-67`; casters in `scene.rs:83-101`, `render.rs` | High: -33 % frame time when skipped | **Partly**: `NotShadowCaster` on ground and track | **Shadows** Off/Low/Med/High |
| 3 | 32-segment cylinders and cones, ico-5 spheres (~300k tris/frame) | `scenery.rs:162-163`; `players/visuals.rs:35-42`; `combat.rs:54`; `render.rs:17` | Medium-high GPU: ~3x fewer tris | **Yes** (at this camera distance; verify screenshots) | n/a |
| 4 | Effect entity churn and per-frame material swaps | `combat.rs:24,166-174`; `explosions.rs:263-399,449,475` | Medium CPU: fight up to 2x idle main-thread | **Yes**: pooling, write material only on stage change | **Effects** Low/Med/High (trail rate, sparks, smoke, debris, chain) |
| 5 | Default 4x MSAA | `scene.rs:37-48` (no `Msaa`) | Medium on iGPU/mobile | No | **Anti-aliasing** Off/4x |
| 6 | Full-window render resolution | `main.rs:37-43`; `web/index.html` | Medium on large screens | No | **Render scale** 50/75/100 % |
| 7 | HUD text rebuilt every frame | `ui/mod.rs:73-98` | Low-medium CPU | **Yes** | n/a |
| 8 | O(missiles × props) scans, per-frame scratch allocations | `combat.rs:197-213,221-232`; `explosions.rs:209,232-258` | Low | **Yes** | n/a |
| 9 | Flash lights, fog, unused PNGs | `explosions.rs:33-38`; `scene.rs:40-47`; `assets/players/` | Low | Yes (PNGs) | Flash lights on/off (folded into Effects) |

**Suggested video-settings menu:** Shadows, Effects, Anti-aliasing, Render
scale, Forest density, plus a preset (Low / Medium / High) that sets all of
them. Default to High on desktop. Consider auto-dropping to Medium when the
average frame time stays above 25 ms for a few seconds.

## 6. Follow-ups for the next tasks
- To attribute wasm time per Bevy system, build with a name section. Use
  `wasm-bindgen --keep-debug`, or skip `-Oz`'s name stripping and profile
  with Chrome DevTools on a machine with a real GPU and more than 6 GB RAM.
  On this host `wasm-bindgen` is OOM-killed on the 74 MB `web-dev` module
  (3.5 GB peak), and running `wasm-opt -Oz` first then trips a wasm-bindgen
  interpreter panic (`unknown instruction RefFunc`).
- Re-run `scripts/perf/ingame.mjs` with the same arguments
  (`--width 320 --height 180 --throttle 1,4 --seconds 15`) after each fix, and
  compare "main-thread busy / frame" and "draws / frame" against §3.1.

## 7. Visual-neutral optimisations: before and after

Branch `perf/visual-neutral` @ `2342170`, one commit on top of
`perf/web-video-settings` @ `9d8f5d1`.

### 7.1 What changed

| Report item | Change | Where |
|---|---|---|
| #1 track | Every track piece (straight beds, rails, sleepers, corner beds and rails, junction sleepers) is welded into one mesh per material at startup: 3 meshes instead of about 250 entities. The corners no longer use `drawArrays` at all. | `tracks/render.rs`, `batch.rs` (`MaterialBatches::build`) |
| #1 carts | The 22 rigid parts of each cart and rider are welded into one mesh per material (9 per cart), still children of the moving cart root. | `players/visuals.rs` (`cart_model`, with a test) |
| #1 forest | Already done on the parent branch: the unreachable backdrop is baked in 14-unit patches. | `scenery.rs` |
| #2 | `NotShadowCaster` on the arena ground slab and the forest floor. They are the lowest geometry, so their shadow lands on nothing. | `scene.rs` |
| #3 | Forest cylinders 32→16 sides, pine cones 32→24, fern fronds 8; rider heads and hair ico 5→3, hands and eyes ico 5→2; missile exhaust ico 5→2. | `scenery.rs`, `players/visuals.rs`, `combat.rs` |
| #4 | Fireballs and smoke puffs write their material handle only when the stage changes, not every frame. | `explosions.rs` |
| #7 | The HUD text is rebuilt only when the seconds, the scores or the finished flag change. | `ui/mod.rs` |
| #8 | The per-frame `HashSet` in `run_detonations` is a reused `Local`. The O(missiles × props) scan in `strike_scenery` is already bounded by the parent branch, because only reachable props are `Scenery` entities. | `explosions.rs` |

Nothing that affects gameplay changed. Collision radii, timings, spawn
logic, RNG use and system order are all the same.

### 7.2 How it was measured

The dev host cannot link a release (thin LTO) wasm, and during this task it
was also compiling a sibling branch. So these numbers come from a GitHub
Actions `ubuntu-latest` runner that was doing nothing else:

- Three release bundles were built exactly as `scripts/build-web.sh` builds
  them (LTO, `wasm-opt -Oz`): `main` @ `61fd92a` (the §3 baseline),
  `9d8f5d1` (parent branch) and `2342170` (this change).
- `scripts/perf/ingame.mjs` ran with the §3.1 arguments
  (`--width 320 --height 180 --throttle 1,4 --seconds 15`), in two rounds,
  alternating builds within each round.
- The runner CPU is not the §3 host, so compare the columns below with each
  other, not with §3.1. It is the same SwiftShader method, so absolute FPS is
  still not meaningful. Main-thread ms per frame, draws and GL calls are the
  numbers to read.

### 7.3 Results (mean of the two rounds)

Renderer main-thread busy time per frame (ms):

| Window | main `61fd92a` | parent `9d8f5d1` | **after `2342170`** | vs parent | vs main |
|---|---|---|---|---|---|
| menu | 10.4 | 13.7 | **10.3** | -25 % | -1 % |
| idle, 1x CPU | 11.6 | 12.3 | **9.9** | -20 % | -15 % |
| fight, 1x CPU | 13.0 | 12.7 | **11.2** | -12 % | -14 % |
| idle, 4x CPU | 43.3 | 42.4 | **32.0** | -25 % | -26 % |
| fight, 4x CPU | 54.8 | 46.0 | **40.1** | -13 % | -27 % |

FPS and median frame time under SwiftShader, which is CPU-rasterised:

| Window | FPS main / parent / **after** | median frame (ms) main / parent / **after** |
|---|---|---|
| idle, 1x | 1.31 / 1.27 / **1.47** | 759 / 792 / **675** |
| fight, 1x | 1.36 / 1.23 / **1.49** | 725 / 817 / **659** |
| idle, 4x | 1.07 / 0.94 / **1.28** | 917 / 992 / **809** |
| fight, 4x | 1.22 / 1.11 / **1.41** | 817 / 900 / **717** |

Work per frame:

| Window | draws main / parent / **after** | WebGL calls main / parent / **after** | triangles main / parent / **after** |
|---|---|---|---|
| idle, 1x | 474 / 689 / **381** | 11 657 / 13 243 / **8 377** | 288k / 360k / **310k** |
| fight, 1x | 504 / 718 / **419** | 12 656 / 14 253 / **9 700** | 289k / 359k / **311k** |

From `anatomy.mjs --fire`, one fight frame, parent → after:

- Shadow pass: 341 → **219** draws, 664k → **496k** vertices. `drawArrays`
  93 → **0**.
- Main opaque pass: 259 → **141** draws, 521k → **386k** vertices.

Compared with the parent branch, draws are down 42-45 % and GL calls down
32-37 %. Main-thread time is down 12-25 %, and median frame time is down
15-20 % at both CPU rates.

The parent branch already draws more than `main`: about 200 more draws and
70k more triangles per frame. This was not investigated here, but the likely
cause is the backdrop patches and settings changes. This change more than
wins it back: against `main`, main-thread time is 14-27 % lower.

### 7.4 Visual regression check

`scripts/perf/shots.mjs` pins `Date.now`, which seeds the map through
`web_time`, so every build grows the same track and forest. It shoots the
lobby and a frame 12 s into a round at 640×360. The shots were compared with
ImageMagick (`compare -metric AE -fuzz 3%`) on the same runner:

| View | parent vs after | parent vs parent (noise floor) |
|---|---|---|
| lobby | 0 px | 0 px |
| in round | 23 581 px (10 %) | 11 657 px (5 %) |

- In-round frames differ even between two runs of the *same* build, because
  the carts and the bots' missiles are wherever the frame timing put them.
- The diff masks (`docs/perf-shots/diff-playing.png` against
  `noise-playing.png`) show every differing pixel sits on carts, missiles,
  smoke and blasts. Both masks look the same in kind. There are no
  differences on the track, ground, walls, forest or shadows.
- Close crops of the carts, compared side by side
  (`shot-9d8f5d1-playing.png`, `shot-2342170-playing.png`), show the same
  shapes, colours and shading on riders, hats and wheels.

### 7.5 Tests and build

- `cargo test --locked` passes on `2342170`, including the new
  `a_cart_welds_into_one_mesh_per_material`.
- The release web build succeeds (GitHub Actions runs on branch
  `ci/perf-bundles`, which is temporary scaffolding only).
- A round plays normally in every run. The screenshots show the round timer
  counting down, the scores changing (P3 scored in one run), the bots firing,
  and blasts and smoke. Gameplay code is untouched apart from the reused
  `HashSet`. Nothing queries cart or track child entities; the only
  `Children` user is prop wrecking, and props are unchanged.

### 7.6 Not done here (still visual-neutral)

- **Missiles as one welded mesh per owner** (#1): the saving is small, 5
  entities per missile, and they are already instanced.
- **Effect entity pooling** (#4): smoke, sparks and fireballs still
  spawn and despawn.
- **Scratch collections in `detect_hits`** (#8): still allocated every frame.
- **Track `NotShadowCaster`** (#2): left out on purpose. Rails and sleepers
  stand 0.1-0.16 above the bed and cast thin shadows that can be seen, so
  removing them would change the image.
- **Fewer track-corner steps** (#3): not needed now that corners are welded
  and cost no draws.
