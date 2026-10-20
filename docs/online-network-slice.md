# Issue #6, phase 2: authoritative networking vertical slice

This is a **loopback-only development match**, not the private-room release.
The room/lobby/reconnect/deployment acceptance criteria in issue #6 remain open.
No infrastructure, CI publication, or deployed service is changed here.

## Run a real match

Use the normal Rust/Bevy development environment described in the README:

```sh
cargo build --locked --bins
cargo run --locked --bin ryggattack-server -- 127.0.0.1:8787
# Separate terminal, native graphical client:
cargo run --locked --bin ryggattack -- --online ws://127.0.0.1:8787 --name Ada
# Or the real shared adapter without a window:
cargo run --locked --bin ryggattack-net-probe -- ws://127.0.0.1:8787 15
# Browser clients (separate terminal):
scripts/build-web.sh
python3 -m http.server 8080 --bind 127.0.0.1 --directory dist/web
```

Open `http://127.0.0.1:8080/?online=ws%3A%2F%2F127.0.0.1%3A8787&name=Ada`
and a second browser with a different name. Alternatively expand **Online
development match** on the ordinary page and submit its form. The `online`
query parameter / `--online` argument is opt-in: absent it, the existing offline
menu, couch controls, arenas, pause and scoring remain the default.

Each connection owns one of four seats. WASD or arrows choose junction exits;
Space fires. The first connected gamepad's left stick and South button also
feed that same seat. Escape toggles **input suppression**, not global pause.
The status text shows the seat, server tick, names, wins, effects and sampled
acknowledgement delay. Blurring the window or focusing the HTML form sends
neutral intent; automatic movement and vulnerability continue. Click the
canvas to regain gameplay focus. Offline / leave online reloads without the
online parameter; native leave is closing the application.

All four carts are active from service start, even before humans connect.
Empty seats move automatically with neutral steering and no firing; they are
**not bots**. Joining may therefore inherit an eliminated cart until the next
round. Free seats can be reused during a running match. There is no ready
countdown, invitation, admission lock, or reserved identity in this phase.
The match uses the foundation's default forest, seed and first-to-three
configuration. Restart the service and reload/relaunch clients after a match
winner; there is not yet a return-to-lobby or new-match command.

## Authority and protocol

* `Server` owns one `headless::Simulation`. It invokes the existing 64 Hz
  gameplay schedule: real rail traversal, directed collisions (including only
  the leading cart's 0.4 rail-distance rear bump), missiles/hits, destructible
  scenery, powerups, elimination, scoring and round reset. There is **no second
  simplified physics implementation**.
* This harness is windowless, **not asset-free or render-independent**. It
  still constructs CPU-side meshes/materials and executes some visual cleanup.
* `network::ClientMessage` is a versioned JSON Hello followed by sequenced
  steering/fire intent. It has no seat, position, hit, score or winner fields.
  Unknown fields, invalid names, unsupported versions, non-finite/out-of-range
  axes and oversized payloads are rejected. Ownership is the accepted socket's
  server-assigned seat; replayed sequence numbers cannot overwrite newer intent.
* Hello sends the explicit rail edge list. Clients construct those tracks,
  never regenerate the topology from a shared seed. State at 16 Hz carries
  all four carts, acknowledgements, names, wins/round status, effect timers,
  projectiles, pickups, destructible collider props and explosion visuals.
  Carts use stable seat IDs and other replicas use monotonically allocated
  IDs. Bevy Entity handles never cross the wire; transient removal is absence
  from the next full state. IDs are scoped to this service process, not a
  durable room/match identity.
* `net_client::NetworkClient` is shared by native graphical, native probe and
  WASM clients. Construction validates the name; the actual socket and its
  timeout clock start on the first `pump`, after expensive Bevy construction,
  rather than expiring while the first scene is built. Fresh acknowledged
  state consumed after a render stall is not mistaken for a missing ack.
  Its callback keeps only the latest state in a mutex mailbox.
  At most one input awaits acknowledgement, with a 62 ms minimum send period.
  This deliberately bounds the adapter's outgoing application queue, but also
  makes input frequency decrease as latency rises. A local fire tap is latched
  until the next send; it is held intent, not a remote action/event queue.
* `online` installs presentation/input systems only: it does not install the
  offline gameplay, scoring, pause or random-generation schedules. Existing
  procedural cart/rider and rail geometry is reused. Collider props, pickups,
  missiles and explosions currently use simple replicated proxy meshes;
  forest backdrop, terrain styling, particles, audio and the polished offline
  HUD are not reproduced. Cart poses interpolate over 62.5 ms; no prediction,
  reconciliation or extrapolation is claimed. Non-cart bodies snap to state.
* Snapshot replication is not a reliable discrete-event log: a short-lived
  visual effect can exist entirely between snapshots. Gameplay results remain
  authoritative and persistent in the next state.

## Bounds and failure behavior

The service refuses non-loopback bind addresses. It has no origin policy,
authentication or TLS setup and **must not be exposed through a reverse proxy**.
A malicious web origin can still reach loopback services; this is not a
production security boundary or CSRF/origin defense.

There are four active peers, at most eight simultaneous handshake workers,
a four-entry admission queue, one-second HTTP-upgrade socket IO timeouts,
a separate ten-second Hello read grace for WASM scene startup,
1 KiB incoming frame/message bounds, a maximum of eight text inputs per peer
per tick, and a 256 KiB serialized snapshot ceiling. Nonblocking peer sends
that cannot progress disconnect that slow peer rather than enqueue unlimited
snapshots; input is neutralized on removal. One second without a newer input
also clears held actions. A half-open handshake does not block the tick.
The per-operation handshake timeout and worker cap are not a full anonymous
abuse defense; slow byte streams can occupy workers. There is no room-creation
rate limit because room creation is not implemented.

A client handshake/ack that takes more than five seconds becomes a visible
connection timeout. Protocol, full-match and disconnect errors are shown in
status text once the app is running. A synchronously rejected initial name
can currently fail before that HUD exists and only log to the console;
entry-form validation is intentionally minimal.
There is no automatic reconnect, bot takeover, host migration,
empty-room pause/expiry or crash recovery. The last human leaving does **not**
pause this development match. These are later issue #6 phases, not partial
implementations of the final product contract.

The server caps wall-clock catch-up debt and logs overruns. Sustained overload
therefore slows simulation rather than pretending all missed ticks ran. JSON
full states are intentionally simple and relatively expensive; bandwidth and
CPU need measurement and optimization before Internet/public service use.
A snapshot exceeding its internal budget currently aborts the server with an
assertion rather than silently truncating gameplay. There is no graceful
shutdown contract or operational health/metrics surface yet.

## Repeatable verification

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets
python3 scripts/headless-smoke.py
node scripts/network-sockets.mjs
# Build native bins and dist/web first; install Playwright Chromium separately.
PLAYWRIGHT_NODE_MODULES=/path/to/playwright/package.json \
  node scripts/network-smoke.mjs dist/network-evidence
```

`network-sockets.mjs` runs the real service and checks stale input, seat and
position spoofing, oversized/binary payloads, invalid axes/version, input flood,
half-open upgrade, and a healthy connection after abuse. Rust socket tests
also cover distinct seats, fifth-client rejection, released seats and the
shared native adapter. Read waits are bounded so regressions fail, not hang.

`network-smoke.mjs` runs **two actual rendered Bevy WASM clients**, the real
service and the native shared-adapter probe. A bounded TCP delay proxy adds
75 ms each way. Browser socket events verify real keyboard input, compare
canonical full-state hashes at common ticks across all three platforms
(normalizing floats to the protocol's `f32` precision, not comparing JSON text),
measure wire input-to-ack delays, and check menu/form/canvas-blur suppression
while the server continues. Playwright forces every headless page focused, so
bringing another page forward does **not** prove OS-window blur; the harness
uses a real DOM canvas blur with held fire instead. Window-focus gating also
has a native predicate regression, but an actual OS-window blur is a remaining
manual/runtime coverage gap. Initial GPU submissions are allowed to yield for
two presentation frames before the online app first polls/opens its socket;
otherwise software shader startup can consume the connection deadline before
browser WebSocket callbacks receive a turn. The harness also saves offline
gameplay/pause screenshots from the exact same WASM artifact. Results include
JS/WASM/HTML SHA-256 hashes. Raw output goes to ignored `dist/`; selected
reviewed screenshots, results and full manifests are committed in
[the evidence directory](online-network-evidence/README.md).

Native graphical runtime requires a display/GPU environment; the probe does
not substitute for that visual test. The browser visual test uses SwiftShader
and its frame rate is not representative of a hardware-accelerated client.
In particular wire acknowledgement time and frame-consumed HUD RTT are
different measurements under slow software rendering. Added network delay,
local interpolation and render/input sampling all contribute to perceived
latency. Prediction and a non-stop-and-wait bounded input policy should be
evaluated before calling the Internet experience responsive.

## Verification results

See [the committed evidence report](online-network-evidence/README.md) for
measured commands, times, artifact hashes, reviewed screenshots and failures
repaired before the final complete replay. Passing these checks completes the
development networking tracer bullet only, not the remaining private-room,
reconnect or Kubernetes issue acceptance checklist.
