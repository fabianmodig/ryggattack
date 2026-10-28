# Explosion presentation evidence (2026-10-28)

Start with [the implementation report](../../online-explosion-parity.md).

- `before/`: real baseline online screenshots and original FAILED report. The replay exceeded its capture-inclusive 180-second gameplay budget after three images and force-killed one Chromium at shutdown. These images are visual context, not a passing baseline gate.
- `reference/`: actual unmodified offline-browser quality6 reference, startup provenance report, contact sheet, and a 10-second excerpt from wall-video seconds432–442. The original601-second recording remains in the task worktree under `verification/parity/offline-reference-video/`. No native graphical window was exercised.
- `after-q0/`: actual pinned0 two-browser six-screenshot gate, exit0; source snapshots and complete public wire/input/lobby evidence compressed with gzip. Contact sheet is an unaltered downscaled montage of the six images.
- `after-q6/`: separately labeled actual pinned6 two-browser compositor supplement, exit0; three real compositor JPEGs captured during60seconds. It is NOT a six-screenshot gate pass. Sparse capture shows smoke/scorch and a partial orange boundary flare, not every fire-color stage. Contact sheet is an unaltered downscaled montage.
- `history/`: unchanged reports for the initial automatic-resume failure, the successful but NOT actually pinned-quality replay, and the failed full-quality screenshot attempt. In the middle report, `quality:6` records the requested initial setting; form navigation dropped it. Do not use that report as pinned6 proof.
- `checks/`: actual logs; full Rust220/220, Node160/160 plus room/UI/runtime/headless/chart checks; Clippy exits0 with7 warnings (3 introduced renderer style warnings).
- `performance.json`: public receive-clock throughput and rAF observations. Pinned0 rAF means933/933ms, not GPU timings. The pinned6 rAF arrays were sampled before callbacks accumulated and contain no samples: null is unavailable, not zero cost. Compositor delivery rate is not FPS.
- `harness/`: exact pure wire classifier/helpers used to independently recompute shared-state evidence; the complete executed source is retained in each candidate source archive.

Run:

```
node docs/evidence/explosion-parity-2026-10-28/verify.mjs
node docs/evidence/explosion-parity-2026-10-28/verify.mjs --checkout
node docs/evidence/explosion-parity-2026-10-28/verify-negative.mjs
```

The first command verifies package completeness/hashes, archived source hashes, screenshots, full shared public-state equality, conservative primary/wall/chain/removal correlations and cleanup records. Report checkpoints are recomputed at their recorded last ticks, while the later capture/Leave tail is independently checked as part of the full retained stream; their counts are not conflated. The second additionally checks current source and the locally retained compiled native/WASM/web artifacts. It requires the shared native cache and dist/web to remain present. Neither command proves compiler provenance from hashes alone or measures GPU mesh vertices; the real build logs and Rust source-to-wire-to-replica vertex/material regressions provide those separate parts.

Known private-value scans and visible-DOM checks ran before publication; image bytes are not OCR. No admission/reconnect credentials, private TLS keys, browser endpoints or raw Welcome messages are included. All rooms/backends were loopback-owned. No public-room test, merge, deployment or production restart was performed.
