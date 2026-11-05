# Immersive mobile gameplay · PR 24

Historical evidence for PR 24 follows unchanged. Its coarse-pointer-only scope
and desktop reserved-band contract are superseded by
[shared gameplay layout](shared-play-layout.md).

## Contract

On coarse-pointer phones, active offline and online play uses the **entire
viewport canvas**, in portrait and landscape. No top score/info band, bottom
control row, or online HUD/diagnostics overlays remain visible. One circular
48×48 CSS-pixel pause affordance floats inside the safe-area insets, with an
accessible name and visible keyboard focus. No orientation lock is requested.

Offline scores/effects come directly from the existing ECS text publication in
the pause dialog, alongside rules, Resume and Return to Menu. The online input
menu uses the existing authoritative HUD model for scores/effects and supplies
rules. **Online input pause never pauses the server.** Finished-match restart
controls and desktop HUD/layout remain unchanged. Swipes/taps still enter the
existing local/online intent adapters; gameplay and compiled Rust are unchanged.

## Provenance and retained evidence

Private evidence root:
`/home/hermes/.hermes/cache/scratch/mobile-immersive/`.
The complete tested bundle is `bundle/`, not `dist/web` and not a new compilation.
`provenance.json` proves all **49** tracked compiled/assets/build-helper inputs
match the retained compiler source receipt (original compiler commit
`49cba9281bf7a06c8fb221a7619a244009e702cd`, compared again with baseline
`73105173947f6a823335db077cd9692acbfa7818`). The original receipts and earlier
evidence were not overwritten. `scripts/wasm-bindgen-version.sh` is explicitly
included in the equivalence inventory.

This is **mixed provenance**: unchanged retained generated bindgen JS and WASM,
with the current authored `index.html` and `touch-controls.mjs`. WASM SHA-256:
`89f37307cddbfb9a5855e2227b6fa8cc5f27e8103cb3035b1534d4f806a81945`.
`executed-source.json` and `executed-source/` bind the current verifier/runtime
bytes; failed touch runs retain their own exact executed verifier. Existing
native server/probe/headless binary hashes are recorded separately, with no
claim of fresh native compiler attestation.

## Verification

- Expected RED: `red-offline-observed.txt` failed because mobile active scores
  remained visible; `red-rendered/results.json` failed on canvas `y=124` rather
  than `0`; `red-online-unit.txt` failed because input-pause scores were absent.
  Each corresponding test was rerun GREEN. The initial `red-offline.txt` is an
  unrelated launcher failure (`/usr/bin/time` unavailable), not TDD evidence.
- `offline-final/results.json`: passed (153.99s), real rendered three-bot gameplay
  at 375×740, 740×375, 320×640, then 375×740 again. Actual backing pixels match
  the full viewport and only `touch-pause` is visible. Actual firing cooldown,
  paused ECS scores/rules, resume and rotation are checked. Desktop 1280×720
  retains its 124px HUD and 80px control reservations; backing is 1280×516.
- `pause-final/results.json`: passed (56.78s), 18 actual WASM/DOM observations:
  W/S (both cases), arrows, Tab/Shift+Tab, Enter, Space, Escape and menu return.
  Emulated standard-gamepad hardware traverses gilrs/ECS for D-pad selection,
  South resume, Start pause and East resume; it does not inject menu selection.
- `gates/results.json`: 38 individual recorded stages, **37 passed and one
  timeout**, not blanket success. The default headless script unexpectedly
  invoked Cargo and timed out after 240s. Its owned orphan Cargo/rustc processes
  were explicitly stopped; `headless-interruption.json` records cleanup.
  `headless-direct.time.json` then passed the same headless behavioral checks
  against the explicit pre-existing executable in 1.26s. This interrupted native
  compile is not a completed build. No new WASM build or optimizer was run.
- Other full non-build gates passed individually: all top-level Node/Python
  test files, prediction evidence checks, Helm/chart contracts, production
  runtime smoke, actual network sockets and whitespace. `final-node.txt` repeats
  current touch/calibration/startup/artifact gates; `final-syntax.txt` checks the
  final mobile harness. Full Cargo test/clippy and a fresh WASM compilation are
  not claimed for this browser-only delta.

### Preserved touch failures

`touch-final-01` failed after 238.26s at **Resume visibility**, not image capture:
a one-bot match naturally returned to Lobby while the input-menu screenshot
was captured. Its four online viewport geometry checks and earlier touch checks
passed, but the complete run failed and remains failed.

`touch-final-02` failed after 238.82s awaiting Running: the attempted zero-bot,
one-human fixture correctly stayed in Lobby because **online solo practice is
not allowed**. This was another failed verifier fixture, not a runtime fix.

The corrected fixture uses one rendered human plus one legal neutral protocol
human, no bots, unchanged gameplay/deadlines. This transport peer is not claimed
as a second rendered browser. Offline natural bot endgame/restart remains a
separate case. `touch-final-03/results.json` **passed in 167.28s**, with 14
interaction steps, 12 retained authoritative shot/cooldown observations and all
four online viewport/backing-size checks. It verifies swipe persistence, real
local firing and authoritative online firing, sampled-fire cancellation,
blur/pause clearing, natural offline completion/restart, online scores/rules,
input-menu resume and explicit Leave. Browser errors were empty. The earlier
failed results remain unchanged; no production deadlines were relaxed.

## Remaining device/release boundaries

Actual iPhone/iPad Safari, physical notches, browser chrome, OS focus and real
hardware gamepads remain unverified. A supplemental CSS safe-area protocol probe
failed because this Chromium lacks `Emulation.setSafeAreaInsets`; its exact
failed diagnostic is `safe-area-results.json`, not product acceptance. CSS still
uses `env(safe-area-inset-*)` for control/menu protection without reserving canvas
space. No production/beta deployment, push or merge is part of this local task.
