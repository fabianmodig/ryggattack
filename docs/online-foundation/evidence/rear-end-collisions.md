# Rear-end cart collision follow-up

Base: `67856d141b09bd29c722239b934bb823a6dee59d`.
Tested `src/players/mod.rs` Git blob: `e2b59733d7746bbed317dc2c9f97cc65e2e8e1ac`.
Only this runtime file changed; remaining changes are documentation/tests.

## Observed RED → GREEN

- `cargo test --locked --lib shared_fixed_tick_rear_end_pushes_only_front_without_reversing -- --nocapture`
  failed before the fix: expected `Edge { from: 0, to: 1 }`, observed
  `Edge { from: 1, to: 0 }` for the front cart. Exit 101, **13.071 s**.
  The same regression passed after the first minimal fix (**34.003 s**).
- `cargo test --locked --lib shared_fixed_tick_rear_end_crosses_bends_and_route_seams_without_reversal -- --nocapture`
  then exposed the incomplete same-path-only fix: expected forward
  `Junction { from: 0, node: 1, to: 2 }`, observed reversed
  `Junction { from: 2, node: 1, to: 0 }`. Exit 101, **18.205 s**.
  Directed successor-route classification made it green; the three targeted
  regressions passed in **14.948 s**.
- Six collision tests subsequently passed (**24.281 s**, including compile).
  They exercise both front-seat/pair orientations, exact ordinary travel plus
  the front-only 0.4-unit rail nudge, actual generated bends and route seams,
  push-through-seam poses, exact overlap tie-breaking, eight ticks without a
  repeated push, inactive/dead/cooling carts, and diverging junction side impacts.
  The prior reversal test now places carts truly head-on, not facing the same way.

The cause was unconditional `reverse_cart` on every close pair, with no rear/front
classification. Rear contact now uses directed route identity/continuity, avoiding
both arbitrary heading cones and free-space translation. Exact overlap has no
physical front, so the higher seat ID deterministically receives the nudge.

## Final native checks

| Check | Wall time | Result |
| --- | ---: | --- |
| `cargo test --locked` | 85.084 s | 126 library + 5 integration tests passed |
| `cargo clippy --locked --all-targets` | 2.670 s | Passed, same four baseline warnings |
| `cargo fmt --all -- --check` | 0.226 s | Passed |
| `cargo build --locked --bin ryggattack-headless` | 0.633 s | Passed, warm after full test build |
| Real no-display executable smoke | 1.928 s | Passed |
| Release web build, bindgen, wasm-opt | 229.578 s | Passed; Rust compile 2m19s, module 53 → 39 MB |

The smoke ran the real executable, with display variables removed, through solo
movement/fire/pickups/scenery and a natural two-cart match: final tick 8192,
wins `[0, 3, 0, 0]`, automatic round restoration, and invalid-argument rejection.
Strict `clippy -D warnings` was also attempted (**5.524 s**, exit 101): only the
same four pre-existing warnings in unchanged explosions/settings/UI code.
Ordinary clippy diagnostics were compared programmatically with the existing
`verification/final-clippy.log`; no new warning was introduced.

The first web command failed in its timing wrapper because `/usr/bin/time` was
absent (exit 127); the build script never ran. The successful retry used shell
builtin `time`. No install, profile change, or redundant rebuild was needed.

## Exact web artifact boundary

SHA-256 after the successful build:

- JS: `bcf993df562a1083f22fbf76f02fbd6f1547917214b635d26680e0f2a090f8f1`
- WASM: `780bf20237b37ad49dae65392da0770927026db6d3c24b110592aaba70af4489`
- HTML: `7ae0de38fb74c7680e01cc429dcee3f74b4bb993c842ad0eb2ed09577503add4`

Raw logs/timings, artifact metadata, and browser captures remain in ignored
`verification/rear-end-*`; no generated bundle or large log is committed.

`node scripts/perf/foundation.mjs dist/web verification/rear-end-browser`
passed on that exact artifact in **246.591 s** using Chromium SwiftShader:
static-path rejection, real keyboard lobby/solo steering/fire, pixel-identical
pause and changing resume, and two keyboard seats, with zero browser errors.
Reviewed `playing.png`, `paused.png`, `two-keyboard-playing.png`, and
`two-keyboard-firing.png`: coherent forest/rails/carts, readable solo/two-seat
HUD, pause overlay, and firing/effects; no obvious rendering regression.
Source and artifact hashes were rechecked unchanged after the browser exited.
The console still includes platform/software-rendering capability warnings;
zero errors does not mean warning-free GPU support.

Limitations: this is not online transport or cross-platform determinism work.
The deterministic shared fixed-tick regressions prove collision behavior;
ordinary keyboard browser smoke is not a forced rear-impact visual test.
