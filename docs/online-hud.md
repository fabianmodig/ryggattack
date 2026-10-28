# Browser online HUD

The browser's online HUD is a presentation layer over validated authoritative
snapshots. It shows active seats only: server-provided player names, `Bot N`
fallbacks, round-win totals, `IN`/`OUT`, and each positive Speed/All-hits timer.
Both effects can appear together. Timer values come from snapshots rather than a
separate browser countdown.

A match result names the authoritative `winner`. A finished round names its sole
active survivor, or reports a draw when no active cart survives. Scores alone do
not determine a winner. Lobby/countdown, room changes and deliberate Leave clear
old results, players and effect timers.

Connection status and notices stay visible without opening diagnostics.
**Diagnostics** is a stable, initially collapsed disclosure containing only tick,
simulation tick and acknowledgement RTT. Private online-room controls remain a
full integrated menu above the HUD in the stacking order. The HUD is concealed
while that menu is visible, rather than competing with its controls. HUD text is rendered
with `textContent`, not HTML; diagnostic/player models do not include reconnect
credentials or invitation metadata.

## Bridge and fallback

`src/online.rs` publishes one serialized `ryggattackHud` value after pumping the
network adapter. It explicitly selects public presentation fields and requires
room/participant/seat agreement before exporting a lobby or frame. Only a
Running lobby may export a frame.

The adapter preserves Welcome and non-Running phase barriers before latest-value
mailbox coalescing. It clears queued and retained/interpolation frames so a
Lobby → Countdown → Running sequence cannot resurrect the previous winner when
several messages arrive between render updates. A subsequent valid State can
supply the new frame. Protocol 3 has no room/match identity in State itself; the
projection relies on validated transport ordering and these lifecycle barriers,
not independent frame-identity proof.

`web/online-hud-bridge.mjs` polls the publication at 100 ms. It does not send
commands, mutate snapshots, handle gameplay keys, or schedule animation frames.
The canvas's legacy online status is suppressed only after the DOM HUD has
successfully consumed a valid publication. Missing/malformed publications retain
the canvas fallback. Native online status and offline gameplay are unchanged;
manual native graphical verification remains deferred.

## Verification

```sh
node --test scripts/online-hud*.test.mjs scripts/refresh-rate.test.mjs
cargo test --locked
cargo clippy --locked --all-targets
cargo build --locked --bins
bash scripts/build-web.sh
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/online-hud-smoke.mjs --local
```

The browser verifier requires the real parent-built `dist/web` and native
binaries. It starts an owned loopback backend and one rendered WASM client. In
the same room/socket, a normally configured target-seven preflight exercises
focus/menu/diagnostic input windows and then completes naturally. A separate
target-one core match starts afterward, using early ordinary held-Space input
without spending its short round on UI prechecks. Case-local snapshots, winners,
inputs, DOM observations and clocks prevent the preflight from satisfying core
assertions. Both verify authoritative/publication/DOM agreement, named results,
same-lobby clearing and fresh match resets; final cleanup sends actual Leave.
Each case retains its own 180-second cap from authoritative Running. The
60-second screenshot allowance applies only to capture and does not extend either
case's cap. The live game uses the established 640×480 SwiftShader viewport;
desktop/narrow geometry is checked separately below.

A **separate isolated synthetic presentation fixture**, after closing the game
page, checks desktop/narrow layout, escaping, coexisting timers, OUT, draw,
diagnostics and room-control focus/clickability. This is not proof of natural
buff collection or a synthetic result occurring in the live game. Screenshot
metadata brackets phase changes; a late capture does not prove an earlier winner.

Results are retained under `verification/online-hud/`, including failed attempts,
source/artifact/helper hashes, fast DOM observations and private-value scanning.
A successful build or unit suite alone is not browser acceptance or deployment.
This HUD slice does not complete effect-scale or prop/projectile/arena visual
parity work.
