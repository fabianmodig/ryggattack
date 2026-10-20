# Issue 6 phase 4 — bounded private-room resilience

Development scope only: loopback WebSocket backend, in-memory rooms, no accounts,
TLS, public reverse proxy, deployment, persistence or production abuse-defense
claim. Web interaction is the priority. Native builds and the same native/WASM
adapter are gates; manual native graphical interaction remains deferred.

## Identity and protocol v3

`Hello { version:3, name, room:null|invitation_code }` still creates/joins a
lobby. A code grants admission, **not an existing participant's identity**.
`Welcome` privately includes `code` and a 64-lowercase-hex `credential`, along
with the existing room/participant/seat/topology fields. Credentials contain
256 bits from the OS random source and are separate from 80-bit invitations.

`Reconnect { version:3, room:invitation_code, participant:id,
credential:private_value }` claims only that participant in that exact room.
No name-based or invitation-only takeover exists. An accepted claim rotates
its credential and replaces the entire former socket, parser, mailbox and
sequence authority. The latest accepted socket starts sequence numbers at one;
queued Input/Ready/Configure/Leave on the superseded socket cannot execute.
Concurrent/replayed old credentials fail. Reconnection never constructs a new
simulation or cart: the existing shared AI/human controller flag changes,
held input and pending fire/turn are neutralized, and position, cooldowns,
buffs, scores, alive/dead state and replica IDs remain intact. Dead characters
wait for the ordinary next round. The active-seat mask is independent of the
controller mask, so substituted bots survive ordinary round restoration.

These are bearer credentials, not accounts or device attestation. Someone who
actually steals the **current** private credential can win the next claim;
rotation rejects subsequent replay, but cannot distinguish that thief from the
owner. A lost rotated Welcome fails closed rather than accepting the old token.
Protecting bearer secrets over the Internet requires phase-5 TLS and endpoint
security; invitation possession alone is never enough. Debug formatting redacts
credentials, service errors never echo payloads, and evidence harnesses remove
private Welcome fields and negatively scan against observed credentials.

## Wall-clock lifecycle decisions

- Transient disconnect reserves the participant/seat for **60 monotonic wall
  seconds** in both lobby and running match. Running disconnect switches the
  existing cart to the actual shared bot AI, without respawning it.
- Admission expires at `now >= deadline`; reclaim is eligible only before the
  deadline. Cleanup runs before admission and independently of gameplay ticks.
  Tests inject `Instant` values; no accelerated production timer or tick-count
  approximation is used for reservations, empty retention or lobby idle expiry.
- Disconnecting host elects the earliest remaining connected human in stable
  admission order. A former host returns as an ordinary participant, unless no
  connected host exists. Simulation authority always remains the server.
- On last-human disconnect, stop calling the simulation immediately. Physics,
  buff, projectile, pickup, round, victory-display and fixed clocks all freeze.
  Keep the empty room for 60 wall seconds; eligible return resumes its existing
  simulation. At the exact empty deadline delete it. Network/service ticks and
  cleanup continue. `Frame.simulation_tick` exposes the frozen gameplay clock
  separately from its monotonic service `tick`.
- A disconnected lobby seat remains reserved and blocks the all-human-ready
  condition. Disconnect cancels the countdown and clears readiness; reconnect
  does not make anyone automatically ready. Expiry releases that lobby seat and
  again clears readiness. Reserved seats count toward the four-seat capacity.
- Expired running claims are permanently revoked for that match. Their bots
  stay active for the rest of the match, including round resets; no new joins
  are admitted mid-match. On return to the same lobby those takeover-only bots
  release their seats. Preserve the original configured bot count, arena and
  win target; clear readiness. The host can configure the next match normally.
- Deliberate Leave revokes the server claim, transfers host, and gives the
  running cart to AI without reserving the former participant. Locally it
  always clears the saved claim and disables retries. If the transport is
  already unavailable, delivery of Leave cannot be guaranteed; the abandoned
  server claim then expires normally, and the local client no longer retains
  its credential. An empty room, even after deliberate leave, is bounded by the
  same 60-second cleanup (with no reclaimable identity if everyone left).
- Non-running idle lobbies expire after 30 minutes of wall time without a room
  action/admission. Automatic held-input traffic does not refresh that timer.
  Running rooms do not use this idle TTL. Server restart discards everything;
  reconnect reports expired/restarted and never silently recreates a room.

The existing four-seat/16-room caps, exact Origin policy, eight admission
workers/four queued admissions, absolute HTTP/Hello/fragment deadlines,
underlying byte/read quotas, message limits and nonblocking slow-consumer
policy remain. Retention can temporarily consume room capacity: the client
gets a capacity error rather than allocating unbounded retained simulations.

## Shared client behavior

Native and WASM use the same bounded retry state machine: backoff up to four
seconds, no more than 12 retry connections or 55 seconds after loss detection,
and a five-second acknowledgement/connection deadline. Terminal protocol,
admission, expired-claim or restarted-backend errors stop retrying. Each retry
uses a fresh bounded mailbox so old callbacks cannot publish stale frames.
Reconnect resets acknowledgement/sequence state, clears interpolation history,
and suppresses held actions until a neutral sample before accepting new input.
Online menus still suppress input, not server simulation; offline behavior is
unchanged. Status text reports connecting, retrying, bot takeover/reservation,
restored seat and actionable terminal errors.

Browsers save identity and private credential in **sessionStorage**, keyed by
exact backend endpoint plus invitation code; tabs are independent. A public
room code (never a credential) is added to the creator's URL so reload can find
that scoped claim. Reload uses the private claim rather than impersonating by
name. Deliberate Leave and terminal server errors remove the saved claim.
The separate “Offline (disconnect)” navigation is a transient disconnect, not
the explicit Leave command; its seat remains reserved until expiry.
Storage-denied browsers retain automatic reconnect only in memory. No credential
is published in the share link, URL, lobby, public diagnostic globals, logs,
console or verification evidence. Native automatic reconnect is process-local;
no native credential file is written.

## Reproducible gates

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets
cargo build --locked --bins
python3 scripts/headless-smoke.py target/debug/ryggattack-headless
node --test scripts/refresh-rate.test.mjs
node scripts/rooms-ui.test.mjs
node scripts/network-sockets.mjs
scripts/build-web.sh
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/resilience-smoke.mjs docs/online-resilience-evidence/reconnect
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/network-smoke.mjs docs/online-resilience-evidence/crossplay
node scripts/rooms-load.mjs docs/online-resilience-evidence/rooms-load.json
```

Run native compilation, full optimized WASM build, rendered replays, and isolated
load **sequentially**. Two SwiftShader games only; close them before offline
smoke. Do not inject a winner or bypass hidden controls. Phase-3 evidence is
historical, not proof for this artifact. Fresh verification results and exact
source/artifact hashes belong in `online-resilience-evidence/`.

The exact 60-second deadline and long-match lobby-idle boundary are exercised
with injected monotonic instants in Rust, not by pretending a backend restart
is a reservation-expiry browser test. The real browser replay uses short TCP
outages, checks held steering/fire remains neutral until release, reloads a
saved seat, and tries the same tab against a different endpoint and room before
restoring the original identity. It closes the second SwiftShader renderer
only after the concurrent takeover/pause/resume checks, before independent
repeated page starts. This keeps rendering contention from consuming the real
reservation while testing storage scope; no production deadline is extended.
Manual native graphical tests remain deferred by the user.
