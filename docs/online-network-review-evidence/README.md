# Network-slice security review replay — October 20, 2026

This evidence belongs to the *review fixes* layered on the original phase-2 evidence in `../online-network-evidence/`. The original evidence is a historical record of the earlier commit, not proof of the final reviewed source. `manifest.json` hashes this replay's exact Rust sources, browser artifacts and native binaries; `results.json` and screenshots are from the actual final Chromium/native run. No runtime source was changed after the final web build and replay.

## Gates

| Check | Result |
| --- | --- |
| `cargo test --locked --all-targets` | 147 passing; 108.736 s |
| `cargo clippy --locked --all-targets` | Success; 3.399 s; existing warnings in unchanged code |
| `cargo build --locked --bins` | Success; 0.441 s |
| `node scripts/network-sockets.mjs` | 16 raw/WebSocket abuse and recovery checks passing; 4.243 s |
| `python3 scripts/headless-smoke.py target/debug/ryggattack-headless` | Passed; 3.193 s |
| `scripts/build-web.sh` | Release WASM + `wasm-opt` succeeded; 183.681 s; 53 MB to 39 MB |
| `node scripts/network-smoke.mjs dist/network-review-evidence` | Two rendered WASM clients, native shared adapter, offline flow passed; 209.062 s |
| `git diff --check` | Passed |

The red/green log timings for Origin rejection, absolute admission deadline, raw frame flood, admitted fragment deadline, post-victory match, topology and state validation are in ignored `verification/timings.tsv`. The admitted-fragment test needed a corrected fixture draining attacker output before its final green run; the earlier false-positive red and intermediate failure are retained in that local log. No dependency or production build-profile changes were made.

## Cross-client observations

* Browser A, browser B and native adapter occupied distinct seats 0, 1 and 2. **1,755 common ticks (3168–10184)** had identical canonical full-state hashes on all three; float canonicalization uses protocol `f32` semantics, not identical JSON spelling.
* The browsers sent real steering and fire inputs; clients observed movement, turns, projectiles, explosions, elimination and round scoring. This run did **not** reach a three-win match. The separately tested server regression verifies a fresh match after a winner, including a new joining client.
* The proxy delayed traffic 75 ms each direction. Browser wire-ack medians were 181.133 / 181.893 ms; the native probe median was 188.289 ms. Sampled on-screen HUD latency is higher and not a measure of input-to-photon response.
* Online menu/form focus and canvas blur neutralized input without pausing the authority. OS-window blur was not tested: Playwright focus emulation cannot prove it. The same WASM artifact subsequently loaded the ordinary offline menu/game/pause with no WebSocket connection. No native graphical window was exercised; the native probe uses the graphical client's shared network adapter.
* The screenshots show both live online arenas and the offline flow. The compact online diagnostic HUD at 640×480 overlaps graphics and has unsupported Unicode glyphs; it is not final UI polish.
* `results.json` has no browser errors, and the server reported no tick-overrun messages in this run. The service remains **loopback-only**, unauthenticated and not for deployment; rooms, ready state, reservations, reconnect and full online visual parity remain future phases.
