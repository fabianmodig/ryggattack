# Matched-quality visual comparison: review correction

This supplement addresses the missing in-frame offline/online progression from PR20's first review. The older `../explosion-parity-2026-10-28` package and failed statuses remain immutable.

## Start here

- [Full-frame paired progression](paired-full-frames.png): offline LEFT, online RIGHT, identical quality0 and960×480 viewport. Four chronological stages: initial yellow, main orange, dark transparent fade, removal/residual smoke. The upper-right wall primary is substantially inside both images, with cart/rail scale references. The fifth row is a separately labeled cream-color supplement; the online cream frame belongs to an EARLIER primary in the repeated volley, not the last primary shown in rows1–4.
- [Identical unscaled detail crops](paired-detail.png): both use the exact source rectangle x590,y40,width200,height170. No independent effect resizing or rotation.
- [Offline sequence](offline-slow-sequence.webm) / [online sequence](online-slow-sequence.webm): actual rendered10-second excerpts. These show ordinary repeated missile/wall bursts, not an injected explosion. The online excerpt also shows nearby smaller prop-chain balls and wreckage.
- Normal-speed supporting clips: [offline](offline-normal-sequence.webm), [online human+bot](online-normal-sequence.webm), [two concurrent rendered online humans](two-human-normal-sequence.webm). These are separate runs, not relabeled as synchronized slow-fixture footage.

## Capture fixture and honest timing

Both comparison columns use original source rendering at pinned quality0 and960×480. Offline uses the preserved, unchanged main20d0594 WASM (SHA in `offline-slow/report.json`); online uses the already-tested db6cae5 candidate WASM/backend (hashes/source records in `online-slow/report.json` and the original evidence package). No game code, shader, material, quality budget, camera, damage, fuse, radius or production deadline changed during this revision. No recompilation was needed.

The widened viewport adds side margins; unlike the rejected640×480 montage, these primary blasts are not clipped by the image boundary. Offline controls are ordinary Enter/W/arrow keys and held Space in zero-bot solo practice. Online uses ordinary room controls, one human and one authoritative bot, held Space and neutral steering. Maps/scenery and participants differ; this compares the same representative upper-right wall-primary progression, not an identical seed, tick, particle trajectory, or pixel replay.

Software rendering skips early colors at normal speed. The detailed comparison therefore uses an EXPLICIT capture-only fixture:

1. Offline: after menu setup, monotonic browser `Performance.now()` advances at0.1× wall time; original simulation/rendering consumes that clock. Date.now is pinned solely for the baseline's existing map seed convention. No gameplay entities or outcomes are injected.
2. Online: the real backend continues at its unchanged64Hz. A loopback FIFO relay forwards original server packets at0.1× their incoming cadence, byte-for-byte, without constructing/modifying snapshots. Outbound ordinary client commands remain immediate. The browser's cosmetic clock advances at0.1× too. `online-slow/report.json` retains received/delivered packet SHA256s/ticks/times:144 delivered State packets have matching bytes and increasing ticks. Undelivered capture tail (1945 queued packets) was explicitly discarded only when capture ended, then ordinary Leave/readback and clean shutdown completed. This is NOT a normal-latency networking acceptance run.
3. Source-video timestamps and exact trim/frame operations are in `provenance.json`. PNGs select the first native decoded frame at or after the named timestamp, without optical interpolation. Contact sheets add labels and use the same full-frame scale/crop on both sides. Wall-time clips do NOT prove native-speed lifetime/fuse/timing equality. Prior source→wire→replica tests remain the timing/geometry oracle.

## Concrete visual comparison

Both columns show a round, slightly faceted single low-quality primary fireball growing through yellow/orange, becoming darker and transparent, then disappearing. At the same camera region and crop scale, the blast is comparable to the cart/track rather than an opaque red disc or undersized projectile. The shared palette is visible, including cream in the separate earlier-shot supplement. The online last-primary capture begins at its observed yellow stage; we do not pretend it contains an unobserved cream frame.

Both retain translucent gray haze after the fireball, with the wall/terrain becoming visible through the dark fade. The extended online clip contains neighboring smaller chain spheres, flying prop pieces and small spark streaks; the normal-speed online clip also shows gray smoke where the orange primary clears. Offline's extended normal-speed clip contains a primary plus nearby smaller bursts and smoke. Density/trajectories do not match exactly: scenery and cosmetic RNG differ, overlapping volleys differ, and offline solo HUD text/fog partly overlays/dulls the upper part of its scene. These are displayed, not masked or color-corrected. The primary outlines remain inspectable in full-frame PNGs and equal crops.

This low-quality pair does not verify full-quality satellite density, flash illumination or every particle trajectory. Original q6 evidence is retained as a separate supplement. No hardware-GPU performance claim is made; time dilation is unsuitable for such a benchmark.

## Actual checks and failures retained

- New widened normal-speed two-human capture: exit0, real two WASM processes, unchanged server-authoritative primary/chain/removal assertions, legal inputs, clean Leave/browser/backend teardown.
- New normal-speed human+bot capture: exit0,503 observed primary frames and346 chain frames, no browser/protocol errors, clean teardown. Match naturally returned to Lobby by the end; only gameplay footage is published.
- Slow visual fixture: exit0,66 observed primary frames and50 chain frames,144 FIFO byte-identical delivered States, no browser/protocol errors, relay/HTTP/backend/browser cleanup successful.
- Offline wide and slow captures: exit0, no browser errors, baseline artifact hashes unchanged.
- Capture fixture regression tests:2/2 pass using a real loopback WebSocket relay plus a monotonic-clock continuity/rate check; original TAP bytes retained in `capture-relay-tests.log`.
- Paired retained verifier:46 manifested files,10 actual source frames,5 clips,3530 retained public states,144 byte-identical delivered packets and4 unchanged client artifacts; exit0. Three independent copy/tamper trials passed, including rejection of changed image bytes and a resigned source/delivery packet mismatch. Original evidence verifier still passes49 files,3544 recomputed shared frames and232 checkout comparisons; the old package has no diff.
- `history/remote-video-api-failure.json`: first video attempt FAILED because remote Playwright does not expose `video.path()`; corrected `saveAs()` used afterward. Its gameplay/cleanup passed but original failure remains.
- `history/one-bot-guard-failure.json`: initial one-bot fixture FAILED an unchanged two-human Configure guard expecting0 bots. Corrected fixture asserts its actual one-bot settings; no product/server rejection was bypassed.

The complete normal/slow metadata and compressed public states/inputs/lobbies are retained. Raw lobby videos are NOT published: selected clips are gameplay-only with disclosures closed, and their images were visually reviewed. Exact-value secret scans do not constitute OCR. No public rooms, deployment, merge or production restart were used.

## Reproduction and verification

`harness/*.txt` preserves the executed harness bytes outside runtime/CI discovery. Restore each online `.mjs.txt` into `scripts/<same-name>.mjs` and use the original helper modules from the PR checkout; run with the three RYGGATTACK_EXPLOSION_* flags shown in the harness and `--local`. Offline harnesses run from the task workspace with preserved `baseline-web/`. Restore the test to the workspace root, where it imports `repo/scripts/capture-relay.mjs`. Paths and installed Playwright package are explicitly host-local, not a portable test-runner claim.

Run `node docs/evidence/explosion-parity-paired-q0/verify.mjs` for retained hashes, packet FIFO/byte integrity, quality, dimensions, metadata and old-package immutability. The original package's independent verifier remains separate. This package's manifest is evidence consistency, not independent compiler provenance or a substitute for visual review.
