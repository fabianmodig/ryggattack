# Verification evidence

All results below are local executions in `/home/hermes/src/ryggattack-online`,
branch `feat/online-simulation-foundation`, based on `a878e5b`.
No remote CI, PR, push, merge, image publication, or deployment was performed.

## Measured results

| Command / check | Elapsed seconds | Result |
| --- | ---: | --- |
| Clean baseline `cargo test --locked`, two-job retry | 275.888 | 108 pass |
| Final `cargo test --locked` | 42.096 | 119 unit + 5 integration = 124 pass |
| `cargo build --locked --bins` | 0.854 | Both native executables build; cached after tests |
| `cargo clippy --locked --all-targets` | 2.303 | Pass, four unchanged-source warnings |
| `cargo fmt --all -- --check` | 0.138 | Pass |
| `node --test scripts/refresh-rate.test.mjs` | 0.255 | 10 pass |
| `python3 scripts/headless-smoke.py` (the CI command) | 3.282 | Real no-display solo and complete two-cart match pass |
| Final `scripts/build-web.sh`, including wasm-opt | 179.136 | Release WASM + JS + assets produced |
| Initial cold web build, including wasm-opt | 449.782 | Pass |
| `foundation.mjs dist/web verification/browser` | 302.692 | Real keyboard play, identical paused pixels, resume, two keyboard seats, no browser errors |
| `shots.mjs`, volcano / one bot / first-to-five | 207.803 | Settings and gameplay screenshots captured and visually reviewed |

Rust commands sourced `~/.local/share/rust-dev-env.sh` after clearing inherited
RUSTFLAGS/LIBRARY_PATH, used the shared CARGO_TARGET_DIR and target symlink, and
ran one cargo build at a time. Browser commands used the existing Playwright
installation with Chromium SwiftShader, 960×540, pinned quality 0. Browser runs
were concurrent with each other, not with Rust compilation; these timings are
verification wall times, **not performance benchmarks**.

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
- `two-keyboard-firing.png`: both carts and a visible orange blast/smoke at the
  lower-right wall while Space remained held.
- `paused.png`: actual PAUSED overlay, RESUME and RETURN TO MENU over gameplay.
  The later paused screenshot has identical bytes; the raw duplicate stays in
  `verification/browser/paused-later.png`. Hash is in `results.json`.
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

- `tdd-red.txt`: ten observed pre-implementation missing-behavior failures.
- `verification.txt`: final test/build/lint/smoke/browser outputs.
- `baseline.txt`: unchanged-baseline OOM and successful retry outputs.
- `commands.tsv`: every recorded command, elapsed time, exit status, and failure notes.
- `timings.tsv`: raw timing rows with a header (44 recorded commands).
- `results.json`: parsed test total, pause hash, and final WASM/JS hashes and sizes.
- `browser.json`: actual console and pause assertion result.
- `headless-selected.jsonl` / `headless-probes.json`: actual runner evidence.
- `changed-files.txt`: exact repository file manifest for this milestone.

The original complete per-command logs and all screenshots remain in
`/home/hermes/src/ryggattack-online/verification/`. Preserve that directory before
removing the feature worktree if more detailed investigation is needed.
