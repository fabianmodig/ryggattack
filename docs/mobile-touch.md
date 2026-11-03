# Phone-friendly web controls

This is a browser feature, not a native iOS application. Desktop keyboard and
controller schemes remain available.

## Playing on a phone

- Choose **PLAY LOCAL**, **JOIN CART**, then **START MATCH**. JOIN CART claims
  the WASD seat. Gestures use that seat (or the arrow seat if WASD is unclaimed);
  gestures themselves never claim additional players. Local bot count, win
  target and arena have full-size DOM buttons.
- Swipe at least 20 CSS pixels on the game canvas to choose a screen-relative
  cardinal direction. The choice survives lifting your finger and remains the
  requested branch at the next junction. Ordinary desktop steering overrides
  this retained fallback while it is pressed.
- Tap the canvas (under 350 ms, without crossing the swipe threshold) to fire.
  A swipe, including one returning to its origin, never fires. A long hold is
  not automatic fire. Taps use the existing simulation cooldown; taps during
  cooldown are not queued for later shots. Several unsampled taps coalesce into
  one pending press, like the existing frame-to-fixed-tick local latch.
- **PAUSE**, **RESUME**, **BACK TO MENU** and, after a completed match,
  **RESTART MATCH** are touch-sized local controls. A local pause freezes the
  simulation. The online **MENU** / **RESUME INPUT** controls suspend your input
  only: the authoritative server keeps running. Online rematches use the existing
  same-room lobby and **READY** button, not a client-side restart.
- Online creation, joining, readiness, settings and room links remain native DOM
  controls; text editing and scrolling are not converted into game gestures.

A second finger cancels the gesture until all participating fingers lift.
That cancellation advances the bridge epoch even if the tap was already sampled
by Rust. It does not reset the active-finger set: lifting only one finger or
adding a replacement finger cannot start a new gesture while another is held.
Pointer cancellation, unexpected capture loss, canvas/window blur, visibility
changes, entering menus and round boundaries clear retained steering and pending
fire. Normal pointer-up / implicit capture loss deliberately retain a swipe.
No keyboard events are synthesized.

## Adapter boundary

`web/touch-controls.mjs` owns pointer recognition and native DOM touch controls.
The one-shot `ryggattackSampleTouch()` callback returns direction, a pending tap,
and a cancellation epoch. `src/input.rs::sample_touch` samples it in `PreUpdate`.
The local human adapter merges it into the existing seat's `ControlIntent`,
retaining fire until a fixed tick consumes it. The online adapter merges that
same intent into its existing outgoing authoritative input and pending-fire
handling. An epoch change clears previously buffered fire on both paths.
Menus enter the existing Rust menu-action, roster and round-restart handlers,
not a parallel gameplay implementation.

The online protocol still carries held intent, not reliable exactly-once fire
commands. As with the existing keyboard adapter, if a severely stalled client
cannot deliver its next neutral sample before the cooldown expires, the server
can fire again at the normal cooldown. A touch tap is recognized/sampled once;
this change does not alter those existing wire semantics or add an action queue.

`touch-action: none` is limited to the active game gesture canvas. Native form
controls keep normal touch scrolling/editing, and viewport zoom is not globally
disabled. Viewport-fit and CSS safe-area insets protect the canvas, DOM menus
and online HUD. Both local and online world cameras widen their vertical field
on narrow windows to preserve the desktop horizontal field, so portrait does
not crop off rail branches. Wide-window framing is restored on rotation. Local
Bevy UI scales for narrow viewports; the DOM toolbar avoids relying
on scaled-down canvas buttons for touch targets.

The optional `ryggattackTouchDebug = true` browser flag publishes read-only local
ECS steering/fire/cooldown observations as `ryggattackTouchObservation`. It does
not modify gameplay or expose reconnect credentials.

## Verification

Run the new module regressions and startup/packaging fixtures:

```sh
node --test scripts/touch-controls.test.mjs scripts/refresh-rate.test.mjs scripts/online-hud-assets.test.mjs
cargo test --locked
cargo clippy --locked --all-targets
scripts/build-web.sh
cargo build --locked --bin ryggattack-server
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json node scripts/mobile-touch-smoke.mjs
```

The browser harness serves only the built local artifact and starts an isolated,
loopback-only development backend with an exact allowed local origin. It uses
Chromium touch emulation and actual CDP touch gestures, plus real outgoing
WebSocket input and opt-in ECS observations. It records artifact hashes, steps
and screenshots under `verification/mobile-touch/browser/`. It never contacts,
publishes to or restarts the deployed game. Source-bound build hashes and TDD
red/green logs are retained under `verification/mobile-touch/` in the feature
worktree. The automated browser gate is separate from Node CI module tests.
The [original acceptance report](mobile-touch-evidence/README.md) is immutable
historical evidence for `c65a509`, not acceptance of the later cancellation fix.
The [cancellation repair report](mobile-touch-cancellation/README.md) retains the
observed-failing sampled-fire regression, corrected browser replay, screenshots,
timings and mixed-artifact provenance. The Node touch suite requires `rustc`:
it compiles actual checked-in Rust sample/local/online transition blocks with
neutral device/transport fixtures, proving pending-fire cancellation between
ticks/sends without changing or rebuilding the production WASM inputs.

Native Apple Safari / physical iPhone or iPad hardware is **unverified**. Chromium
mobile emulation does not establish Safari GPU, real safe-area, address-bar,
keyboard or OS-level gesture behavior. Test those on Apple hardware before
claiming iOS/Safari acceptance.

Reference behavior follows Pointer Events' `touch-action` model and WebKit's
viewport-fit / safe-area design (W3C Pointer Events Level 3; WebKit “New WebKit
Features in Safari 13” and “Designing Websites for iPhone X”).
