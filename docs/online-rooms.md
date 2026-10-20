# Issue 6 phase 3 — historical private room/lobby development slice

This document describes the merged **v2/phase-3** artifact and its historical
evidence. Current v3 reconnect/retention behavior and limits are documented in
[online-resilience.md](online-resilience.md); its policies supersede the
disconnect/clock descriptions below.

This remains a loopback-only, anonymous development backend. No infrastructure,
public bind, deployment, reconnect reservation or crash recovery is included.
See README for browser/native controls; phase-2 evidence is historical.

## Protocol v2 and ownership

`Hello { version:2, name, room:null }` creates a private lobby. A 20-character
lowercase hex `room` joins an existing lobby. Codes use ten bytes from the OS
random source (80 bits), not room IDs or display names. Missing codes, full
rooms and running matches return distinct visible errors. Rooms and humans
have monotonic server-issued IDs, independent of names, simulation seats and
raw ECS handles. Codes grant lobby entry, never another participant's identity.
There is no reconnect/takeover API. Every accepted device gets one seat.

`Welcome` includes room and participant IDs, seat and explicit rail edges.
`Lobby` carries the authoritative host, human roster/readiness, settings and
phase (`Lobby`, `Countdown { ticks_left }`, `Running`). `Ready(bool)`,
`Configure { arena, wins_target, bots }`, and `Leave` act only for the socket's
participant. Unauthorized or state-inapplicable lobby operations return a
recoverable `Notice`; malformed/rate-exceeded input is fatal. Host options
are Forest/Volcano, 1/3/5/7 wins and 0–3 bots bounded by remaining seats.

Human joins replace lobby bots as necessary, reset readiness/countdown, and
never exceed four participants including bots. All humans ready starts 128
fixed ticks at 64Hz; bots never count as ready humans. Unready/disconnect
cancels countdown, and settings/join/disconnect clear readiness. A one-human
zero-bot ready request explains that solo practice is offline. One human plus
shared AI bots starts normally. New match admission is locked even if free
seats remain. Settings cannot mutate a running match.

Each running room owns a separate existing `headless::Simulation`. It runs the
same gameplay chain, real bot AI, rail movement, front-only 0.4 rear bump,
hits, scenery, powerups, elimination, scoring and round restoration. No new
physics implementation or client gameplay simulation exists. Victory remains
visible for 192 ticks, then connected humans return to the same room/code,
settings retained, readiness cleared. Match-local replica IDs may restart
when a fresh simulation is constructed; room/participant IDs do not.

Disconnect or deliberate leave removes a lobby human and elects the earliest
still-connected human (join order, not lowest seat or display name). During
play the disconnected cart remains externally controlled with neutral input;
no AI takeover or reservation is implemented. No new device may join that
running roster. Phase 4 will define secure reservations and reconnection.
There is no round timeout: coasting/disconnected carts or unproductive human
input can produce a genuine stalemate. A match that has not produced a winner
does not falsely return to the lobby. Phase 3 does not claim automatic recovery
or guaranteed completion after a mid-match disconnect.

## Resource/security boundaries

Maximum 16 rooms / 64 admitted devices. Empty rooms are removed immediately.
A lobby without a settings/readiness/join action expires after 30 minutes of
server ticks; automatic adapter input traffic does not refresh it. Running
rooms do not idle-expire. The clock is injectable through explicit `tick` /
`Lobby::advance` calls, not sleep-driven readiness logic. Sustained server
overload slows this clock; the existing bounded wall-clock-debt policy remains.

All phase-2 transport guards remain: exact browser Origin allowlist; native
no-Origin policy; eight admission workers/four queued sockets; absolute
one-second HTTP upgrade and ten-second accept-to-complete-Hello deadlines;
1KiB message/frame limit; admitted fragmented/partial-message one-second
absolute deadlines; per-peer/tick 1KiB underlying-byte and four-read quotas,
plus existing buffered bytes; eight completed messages/tick; 256KiB snapshot
ceiling; nonblocking slow-consumer disconnect; stale sequence rejection and
one-second held-input neutralization. Room commands share those quotas.

This is **not** an Internet anonymous abuse-defense architecture. The room cap
and idle cleanup bound retained state but there is no distributed rate limiter,
TLS/authentication or production metrics. OS randomness is required on this
Linux development service. Simulation still constructs CPU visual assets;
it is windowless, not an asset-free server crate.

The shared adapter validates IDs, code format, unique human IDs/seats, names,
host membership, settings/capacity, countdown bounds, complete connected map,
and all previous frame geometry/timer/id constraints before publication.
Latest-lobby/latest-frame mailboxes retain bounded application state. Input
is only sent while running and remains stop-and-wait. Form/menu/blur suppress
local actions without pausing the room. A fresh lobby discards stale visual
frames, so returning to the lobby cannot carry pending gameplay intent into
another match.

The selected backend must still be trusted for allocation safety: browser
WebSocket messages allocate before callback validation, and native ewebsock
limits individual frames rather than aggregate malicious messages. Do not
expose this service with a reverse proxy.

## Verification

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets
python3 scripts/headless-smoke.py
node scripts/rooms-ui.test.mjs
node scripts/network-sockets.mjs
scripts/build-web.sh
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/network-smoke.mjs docs/online-rooms-evidence
```

Native graphical runtime needs a display and is not substituted by the native
adapter probe. The Bevy native UI code-entry system has windowless ECS coverage;
actual graphical native interaction remains a manual verification gap. The
browser gate uses two real rendered WASM apps plus the same native adapter,
a 75ms-each-way proxy, actual HTML room actions, keyboard gameplay, same-tick
full-frame hashes, and a separate room without a third renderer. Online pages
close before the unrelated offline graphical smoke to avoid SwiftShader
starvation. See fresh evidence and source/artifact hashes in
`online-rooms-evidence/`; no historical screenshots establish current coverage.

The rendered lifecycle fixture pins the existing first-room seed3 (room ID3)
and selects **first to one** through the real host UI, not a result hook. It
keeps the native participant connected until the gate finishes and uses actual
Left/Fire and Fire keys for completion. Real sampled-input characterizations
cover startup delays without injecting a winner or changing world state.
Further input checks can ready a new match in the same lobby after a genuine
victory. The original three-win spontaneous-play fixture could stalemate and
let its native helper expire; that failed evidence is retained, not hidden.
Multi-round first-to-three remains covered by the real headless smoke and
shared simulation tests. Timeout diagnostics now include full last frames,
room phase/roster, input tails and native exit timing.
