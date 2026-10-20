# Phase 4 resilience verification — October 20, 2026

This evidence is from the final **review-corrected** runtime, not an earlier successful prototype run. `manifest.json` records runtime source hashes, test/harness hashes, native binary hashes, browser-artifact hashes and timings. No runtime source changed after the final release build and the replays below. Native graphical interaction is explicitly deferred by the user; the web client is the priority.

## Final gates

| Gate | Wall seconds | Actual result |
| --- | ---: | --- |
| `cargo test --locked --all-targets` | 139.644 | **181 passed** |
| `cargo clippy --locked --all-targets` | 3.655 | Passed, four existing warnings in unchanged code |
| `cargo build --locked --bins` | 0.426 | Passed |
| Real headless gameplay smoke | 1.269 | Passed, including a completed three-win match |
| Refresh-rate calibration / fallback tests | 0.338 | 10 passed |
| Room UI helpers / validation | 0.044 | Passed |
| Reconnect harness initialization regression | 0.128 | Passed |
| Real socket-abuse / recovery harness | 10.356 | 16 passed |
| Release WASM build plus `wasm-opt` | 184.368 | Passed; rounded module size 53 MB to 39 MB |
| Actual rendered reconnect replay | 196.381 | All 7 scenario groups passed |
| Actual rendered crossplay + offline replay | 306.035 | All 19 checks passed |
| Separate one/16-room load | 75.157 | Approximately 64 Hz; zero overrun warnings |

Final gate output is preserved under `logs/`. `logs/timings.tsv` retains the iteration history, including failed RED tests and unsuccessful earlier gates. Failed graphical attempts remain locally under ignored `verification/`, not represented as final passes.

## What the actual browser reconnect replay exercised

Two real WASM/Bevy clients played through the real loopback service and independently cuttable TCP proxies. No character, score, winner or credential-acceptance response was injected.

- A real TCP cut caused the existing character to switch to actual shared AI. The other browser displayed **Bot takeover**, a decreasing reservation and the replacement host.
- Automatic reconnect restored the same room, participant and seat. The returning former host did not displace the connected host. Common post-reconnect full frames matched between the two transports, and the actual rendered loop continued.
- Steering and fire were held before and across the outage. Three observed post-reconnect input packets remained neutral until release. After actual released-key presentation updates, a new steering press produced a non-neutral outgoing input.
- Both transports were cut. Across a measured **5,571 ms** wall gap, the first returned simulation clock advanced only **12 ticks**, including detection/snapshot-boundary work, rather than simulating the missing wall time. Exact freeze/state preservation is also covered by injected-clock Rust tests.
- Reload reclaimed the saved private seat. The same tab then used another endpoint and another invitation; both sent fresh **Hello**, not the private reconnect claim. The original stored claim remained present, and returning to its exact endpoint/code restored the original identity. The two unrelated-admission checks took approximately 12.3 seconds in the final run.
- An actual backend process restart produced terminal expired/restarted feedback, cleared the saved claim and did not silently recreate the room. Actual incompatible-version, missing-invite, room-full and unavailable-backend paths were exercised. Deliberate Leave cleared storage and stopped retries.

`reconnect/results.json` records the observations and reports no unexpected browser errors, **7 expected WebSocket console errors** from intentional outages, zero server overrun warnings, and a successful negative credential scan. Private credentials were removed before evidence serialization. No credential is part of a screenshot, share link, URL or public diagnostic state.

The two-renderer takeover/pause/resume gates finish before closing the second renderer for independent reload/scoping/error scenarios. Earlier repeated full-page starts with both SwiftShader renderers active produced an expired/invalid claim; the final fixture keeps the real 60-second production deadline unchanged and records storage presence/elapsed times. An earlier fixed-duration key-release/press fixture also missed input sampling at software-rendering speed; it now waits for actual presentations and outgoing input rather than assuming a sleep contains a frame. Both failed attempts are retained locally.

## Crossplay, local regression and load

`crossplay/results.json` records **1,404 identical canonical authoritative snapshots** at shared service ticks **4312–12364** across two actual rendered browser clients and the native shared adapter, with **75 ms delay each direction**. Real keyboard steering/fire, elimination/scoring, a natural first-to-one winner, return to the same lobby, invitation/host controls and offline menu/play/pause all passed. Float comparison follows protocol `f32` semantics, not identical JSON decimal spelling.

Browser wire-ack medians were approximately **176.3 / 180.5 ms**, native **188.5 ms**. These are not input-to-photon figures: the software-rendered HUD samples show substantially greater latency. The final crossplay run had no unexpected browser errors or server overrun warnings and passed its negative credential scan.

`rooms-load.json` measures two separate 20-second steady windows without browsers or compilers: one human plus three bots, and 16 isolated rooms with 16 humans plus 48 real bots. Minimum observed rates were **64.000264 / 64.000497 Hz**; zero overrun warnings. This is local development-service evidence, not an Internet capacity or latency guarantee.

Reviewed screenshots show actual running carts/rails, bot-reservation and restored-seat status, saved-seat reload, scoped identity restoration, returned lobby, and ordinary offline pause. The native probe automatically readies again after a match; the returned-lobby screenshot may show it ready even though the recorded immediate authoritative transition cleared all readiness. Screenshot/UI updates are asynchronous: a status screenshot may show a newer canvas HUD than the HTML polling panel. Online collider scenery proxies and the crowded diagnostic HUD remain prototype-level visual limitations.

## Review corrections and boundary coverage

Independent review found four defects, each corrected after observed failing regressions:

1. A fresh human joining a retained hostless lobby now elects a connected host, without consuming a reserved identity; transient disconnect and deliberate-leave cases include real shared-adapter admission.
2. Match completion starts a new lobby inactivity window. A long match cannot immediately delete its returned lobby using the pre-match activity timestamp.
3. All terminal identity/protocol/server errors use common scoped-storage cleanup, clear the in-memory credential, close/remove the transport and discard stale lobby/frame state.
4. Accepted Welcome clears a stale command notice so it cannot mask restored status.

A targeted independent rereview found no remaining blocker in these corrections. Exact 60-second expiry/reclaim boundaries, dead-character/buff preservation, expired takeover across round resets, superseded socket/credential rejection, room isolation and cleanup are tested through injected monotonic instants and real sockets. **The browser replay does not claim to wait 60 seconds to test expiry**, and backend-restart feedback is not substituted for that test.

## Remaining scope

The service remains loopback-only and in-memory. There is no backend deployment, TLS, public admission/rate-control architecture or crash persistence. Phase 5 remains open. A current stolen bearer credential is inherently indistinguishable from its owner; rotation rejects old/replayed/wrong/expired/cross-room credentials, not a thief who already possesses the current private secret. See `../online-resilience.md` for the explicit threat model and lifecycle policies. Native graphical interaction and OS-window blur remain deferred/unverified; native builds, shared-adapter crossplay and windowless UI/input regressions passed.
