# Verification evidence

All results below are local executions in `/home/hermes/src/ryggattack-online`,
branch `feat/online-simulation-foundation`, based on `a878e5b`.
No remote CI, PR, push, merge, image publication, or deployment was performed.

## Exact graphical coverage boundary

The full guarded Chromium run (246.403 seconds) covers source commit
`3f452b169dc0cecb459da363705ac38473513f4a`. Forest solo pause/resume,
two keyboard humans, static-path rejection, and volcano captures were reviewed.

**After that playtest**, only two runtime source files changed:
`src/input.rs` adds a one-tick local fire latch, and `src/players/mod.rs`
neutralizes paused/finished input and samples that latch (plus two regression
tests). Those fixes passed the complete 126-test Rust suite, actual headless
matches, native builds, clippy/fmt, and a fresh full WASM build. **The newest
WASM was not graphically replayed after these narrow input fixes.** This is
explicitly a remaining review/coverage boundary, not a claim that the final
commit received the earlier full graphical playtest. `results.json` records
both artifact hashes and before/after source blob IDs.

## Measured results

| Command / check | Elapsed seconds | Result |
| --- | ---: | --- |
| Clean baseline `cargo test --locked`, two-job retry | 275.888 | 108 pass |
| Final `cargo test --locked` | 43.895 | 121 unit + 5 integration = 126 pass |
| `cargo build --locked --bins` | 0.644 | Both native executables build; cached after tests |
| `cargo clippy --locked --all-targets` | 2.731 | Pass, four unchanged-source warnings |
| `cargo fmt --all -- --check` | 0.132 | Pass |
| `node --test scripts/refresh-rate.test.mjs` | 0.258 | 10 pass |
| `python3 scripts/headless-smoke.py` (the CI command) | 3.078 | Real no-display solo and complete two-cart match pass |
| Final `scripts/build-web.sh`, including wasm-opt | 178.284 | Release WASM + JS + assets produced |
| Initial cold web build, including wasm-opt | 449.782 | Pass |
| `foundation.mjs dist/web verification/browser-final` | 246.403 | Pre-local-latch source: real keyboard play, identical paused pixels, resume, two keyboard seats, no browser errors |
| `shots.mjs`, volcano / one bot / first-to-five | 207.803 | Settings and gameplay screenshots captured and visually reviewed |

Rust commands sourced `~/.local/share/rust-dev-env.sh` after clearing inherited
RUSTFLAGS/LIBRARY_PATH, used the shared CARGO_TARGET_DIR and target symlink, and
ran one cargo build at a time. Browser commands used the existing Playwright
installation with Chromium SwiftShader, 960×540, pinned quality 0. Some earlier
browser runs shared CPU with other local verification. These
are verification wall times, **not performance benchmarks**.

The first foreground baseline exceeded its 420-second tool window with no
completed result. A subsequent tracked uncapped baseline suffered an OOM
SIGKILL after 192.384 seconds on the original 8 GiB/no-swap allocation. After
confirming no orphan compilers, the explicitly authorized two-job retry passed.
The parent then raised the allocation to 12 GiB; later builds used normal
parallelism. No production Cargo profile was changed. The OOM counter remained
at two during the subsequent verification.

One intermediate GREEN attempt hit Bevy's 20-system tuple limit and was fixed
by nesting the input chain; this was a compile failure, not a test result.
Strict `clippy -D warnings` also failed on four existing warnings in unchanged
explosions/settings/UI source. Ordinary final all-target clippy passed with
exactly those warnings. These failures are retained in `commands.tsv`, not
hidden or relabeled as successful runs.

## Actual headless state changes

- Solo seed 42, 2048 ticks: carts move, real missiles fire, scenery count falls
  from 138 to 125, and a real rail pickup appears at tick 1920. Wins stay zero.
- One human + one bot, seed 42, 8192 ticks: naturally reaches `[0, 3, 0, 0]` wins;
  the winning third point is visible by the 28-second diagnostic sample.
  Intermediate samples show both carts restored after the first two wins.
- The separate three-bot, 128-second probe eliminates two carts but reaches no
  round win in that interval. This harness does not improve the existing bot
  AI or promise every seeded bot encounter will finish quickly.

`headless-selected.jsonl` contains seven **selected actual snapshots**, not
synthetic examples, from the one-bot run: initial state, win changes, round
restorations, and final state. `headless-probes.json` summarizes both additional
probes. Full original logs remain under ignored `verification/`.

## Browser observations

Images were inspected, not accepted solely because a script exited zero:

- `two-keyboard-lobby.png`: P1 WASD, P2 ARROWS, zero bots, forest, first to three.
- `two-keyboard-playing.png`: two carts and the two-player elimination HUD.
- `two-keyboard-firing.png`: both carts, with smoke/scorch near the lower-right
  wall while Space remained held. No visible missile is asserted from this still.
- `paused.png`: actual PAUSED overlay, RESUME and RETURN TO MENU over gameplay.
  The later paused screenshot has identical bytes; the raw duplicate stays in
  `verification/browser-final/paused-later.png`. Hash is in `results.json`.
- `volcano-playing.png`: basalt/lava scene, carts, and first-to-five two-player HUD.
  The original settings screenshot confirms one WASD human and one bot.

`browser.json` records zero page errors and zero console-error messages. It
also preserves Bevy WARN-level logs about software/WebGL capabilities and one
camera-without-render-graph warning. **Zero errors does not mean zero warnings.**
Rendered gameplay and UI were visibly functional; the graphics settings module
was not changed. Native display/GPU was unavailable: native builds and windowless
execution passed, but real native graphical/gamepad play was not performed.
Browser screenshots do not prove powerup collection; shared ECS tests and
no-display execution cover powerup rules separately.

## Files

- `tdd-red.txt`: thirteen observed pre-implementation failures, including the
  two local input regressions and the new loopback test server's path boundary.
- `verification.txt`: final test/build/lint/smoke/browser outputs.
- `baseline.txt`: unchanged-baseline OOM and successful retry outputs.
- `commands.tsv`: every recorded command, elapsed time, exit status, and failure notes.
- `timings.tsv`: raw timing rows with a header (59 recorded commands).
- `results.json`: parsed test total, pause hash, pre-playtest/current artifact
  hashes, exact graphical source commit, post-playtest source blobs, and static scan.
- `browser.json`: actual console and pause assertion result.
- `headless-selected.jsonl` / `headless-probes.json`: actual runner evidence.
- `changed-files.txt`: exact repository file manifest for this milestone.

The original complete per-command logs and all screenshots remain in
`/home/hermes/src/ryggattack-online/verification/`. Preserve that directory before
removing the feature worktree if more detailed investigation is needed.
