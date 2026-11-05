# Shared gameplay viewport

Local and online play use the same full-viewport canvas at every pointer type
and viewport size. Safe-area insets protect menus and the shared 48×48 floating
pause affordance, not the render surface. There are no reserved HUD/control
bands. Scores, IN/OUT, effects, rules and controls remain available in pause;
online input pause does **not** pause the authoritative server. Keyboard,
gamepad, touch gestures and finished-match restart actions are retained.

## Cause and implementation

The previous local-only `.offline-playing` CSS reserved **124px above and 80px
below** the actual Winit surface. Only a `(pointer:coarse)` override removed
those reservations. Narrow/fine-pointer local play therefore had a smaller
canvas than online play, regardless of orientation. Exact-main Chromium
reproduction at 375×740 measured local CSS/backing **375×536, y=124** versus
online **375×740, y=0**; at 740×375, local backing was only **740×171**.
Coarse-pointer main already filled the viewport in this reproduction. This
is not a claim to have reproduced the user's physical iPhone/Safari behavior.

`#game` now has one viewport rule. The browser bridge publishes one `gameplay`
class for either local Playing or authoritative online Running, and both modes
use the same `.game-pause` styling. Gameplay no longer depends on pointer media
queries; those remain only for touch lobby controls. Score/help presentation
is paused-only for both fine and coarse pointers. The old desktop reserved-band
assertions were replaced rather than preserving unwanted empty space.

The existing shared `fit_world_camera` in `src/settings.rs` already preserves
the reference horizontal field in portrait and restores the original vertical
field on wide windows, for local and online. It is unchanged. Viewport backing
checks and rendered screenshots cover the resulting framing, not only DOM bar
visibility.

## Replaying locally

Use a real packaged web bundle and an existing matching native server executable
(or build those through the normal reviewed build path):

```sh
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
LAYOUT_BUNDLE=/path/to/bundle \
LAYOUT_EVIDENCE=/path/to/evidence \
LAYOUT_SERVER=/path/to/ryggattack-server \
node scripts/shared-play-layout-smoke.mjs
```

The harness serves only loopback, rejects path traversal/malformed requests,
and never contacts production. It exercises portrait 375×740, narrow 320×640,
landscape 740×375, desktop 1280×720 and rotation back, with both pointer types,
in both modes, plus local → menu → online → menu → local transitions. Online
uses **one rendered human and one legal neutral transport human**, not two
rendered clients. The transport participant remains active until teardown;
there are no bots or injected winners/gameplay snapshots. Input checks include
real local cooldown, trusted keyboard steering, and online swipe/tap reaching
authoritative cooldown. Scores and help remain available through real menus.
`LAYOUT_BASELINE=1` records all layout failures instead of stopping at the first,
allowing comparison against the old main bundle.

Private evidence root:
`/home/hermes/.hermes/cache/scratch/shared-play-layout/`.
The retained exact-main baseline is `baseline-bundle/`; its RED report and
executed verifier are under `red-rendered/`. `red-unit.log` and `green-unit.log`
bind the focused fine-pointer regression. `candidate-provenance.json` records
unchanged compiled inputs and all candidate bundle hashes. This is **mixed
provenance**: published release-105 generated JS/WASM from source `283b379`,
verified against its retained public/image receipt, plus current authored
HTML/modules. No new WASM compilation is claimed. The pre-existing native
server's hash is recorded separately, without a fresh compiler attestation.

`gates/results.json` records **28/28 passing** individual exits/timings for the
allowlisted non-build CI gates (summed command time 42.166s). The focused unit
regression failed as expected, then passed; the combined touch/calibration gate
passes 23 tests. Exact-main rendered RED completed in 372.897s: 22 measured views,
11 fine-pointer failures, with coarse-pointer views already passing. Candidate
rendered GREEN completed in 415.882s: all 22 views, real local touch cooldown,
online swipe/tap plus authoritative cooldown, fine-pointer keyboard steering,
and both complete mode-transition paths passed, with no page errors. The 26
retained captures were visually reviewed via a contact sheet, with full-size
portrait, desktop, online landscape and online pause checks; rails remain
framed and active info/control bands are absent. `pause-rendered/results.json`
passed 18 real WASM/DOM keyboard and emulated-hardware gamepad observations in
55.062s, including W/S, arrows, Tab/Shift+Tab, Enter/Space/Escape, D-pad selection,
South/East resume and Start pause. This is API-emulated gamepad coverage, not a
physical controller check. The updated `offline-hud-smoke.mjs` also passed in
164.631s: four mobile/rotation views, real firing, pause scores/help and keyboard
focus checks, plus desktop CSS/backing **1280×720**, without the obsolete bands.

The no-argument headless helper was deliberately avoided;
the existing executable was passed explicitly. Full Cargo test/clippy, a fresh
web build, remote exact-head CI and actual iPhone Safari remain separate gates.
The attempted `/usr/bin/time` launcher is retained as exit 127 tooling failure,
not expected product RED; the baseline replay uses shell `time` instead.

The host Chromium lacks `Emulation.setSafeAreaInsets`. The supplemental
`safe-area-layout.json` uses **synthetic env() substitution**, checking portrait
47/34px and landscape 44/21px insets for both modes/pointers without WASM. It is
not physical safe-area or Safari evidence. Actual iPhone notches, address-bar
resize and Safari behavior still require device verification.
