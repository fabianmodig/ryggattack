# Mobile touch verification

Final local acceptance: **257 Rust tests**, **166 selected Node feature/UI tests**,
**10 real-browser checks**, and **six visually reviewed 375 × 740 screenshots**.
The native Clippy gate passes with seven warnings, matching a fresh check of the
unchanged `5274202` baseline source (copied only into ignored verification data).
Room UI, calibration/startup, packaging, TLS, server policy/banner, production
socket/abuse, headless executable, container-network/build contracts, Helm lint
and chart contracts also pass locally. No publication or deployment was run.

## Exact artifact

- `build-source.json` records the base Git revision and hashes of the tested
  dirty source files. Those hashes match the committed runtime files; the
  final browser run independently checked every entry.
- `browser-results.json` records the exact executed smoke verifier hash,
  local JS/WASM artifact hashes, the passing steps and real authoritative
  firing cooldown observations. The test did not synthesize keyboard input.
- The final rebuilt release artifact is in ignored `dist/web/` in the feature
  worktree. Its optional `wasm-opt` pass was deliberately omitted for local
  acceptance; the preceding release build did run the optimizer. Production
  build defaults were not changed.
- `verification.json` records measured timings and reviewed screenshot hashes.
  Final native tests: 208.987 s; Clippy: 7.775 s; final WASM build/package:
  572.826 s; final full browser replay: 121.787 s. The earlier optimized build
  took 646.093 s. Timings for the earlier TDD builds/replays are retained too.

## Coverage

Real touch gestures exercise retained steering, no swipe-triggered firing,
local ECS cooldown fire, cancel/multitouch, actual DOM canvas blur, keyboard
regression, local pause/resume, a naturally completed first-to-one match and
restart, private online creation/readiness, real outgoing swipe/tap input,
authoritative server firing cooldown, and online menu/resume neutral input.
Offline map seed 3 is pinned through the clock, not by injecting a winner or
modifying simulation results. Screenshots wait for actual DOM Resume/Restart
visibility rather than only the earlier Rust phase publication.

The portrait replay initially exposed cropped rails/carts. The shared camera
now preserves the reference horizontal field at narrow aspect ratios; the
observed-failing camera regression and final rebuilt local/online screenshots
cover that correction. Large DOM controls replace reliance on the deliberately
scaled-down legacy canvas lobby buttons/text for phone actions.

Representative red logs and final green native/Node/browser/build logs are
included. Archived red logs retain their original bytes, including Node TAP's
indented blank lines; the local `.gitattributes` whitespace exception applies
only to those immutable `*-red.txt` records, never source files. Full logs,
failed/superseded replays, original pre-camera artifact and
additional CI-contract outputs remain under ignored `verification/mobile-touch/`
and `dist/web-before-camera/` in the feature worktree. They have not been
relabeled as final acceptance or selected for publication.

One rejected harness tap was an actual 638.5 ms hold under software rendering,
above the intentional 350 ms tap limit. `tap-diagnosis.json` records the traced
boundary; using Playwright's trusted atomic tap fixed the verifier without
changing product gesture timing. Another verifier race sent online gestures
before the DOM had left the lobby; the final run requires the visible playing
surface and canvas focus first.

## Boundaries

**Native Apple Safari / physical iPhone or iPad verification is unperformed.**
Chromium mobile emulation does not verify physical safe areas, mobile keyboards,
OS-window blur, address bars or Apple GPU behavior. Actual DOM canvas blur and
unit-level visibility/window-blur cleanup are covered separately.

Online wire intent remains the existing held-state protocol, not exactly-once
fire commands. A severely stalled client can permit repeat fire at the normal
cooldown until neutral input reaches the backend; this is not a new action queue.
The final optimized variant of the corrected artifact was not browser-tested;
the unoptimized release artifact was. Container image build/publication and
real-device performance were not part of this local acceptance.
