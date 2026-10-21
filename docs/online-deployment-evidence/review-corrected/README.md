# Corrected phase-5 local verification

This directory is packaged only after the corrected-source gate succeeds. It is
**local execution evidence, not public deployment or trusted public TLS proof**.
The parent evidence directory preserves the earlier pre-review runtime results;
those do not substitute for these reruns.

## Source and artifacts

- Runtime/deployment source is `4136c80b24291235c3b9e8660d7012aeb7bc7c00`.
- The two review blockers are corrected: lobby waiting no longer consumes the
  Running input-idle window; nginx request-context errors cannot retain private
  query/Referer strings, while sanitized access and global startup logs remain.
- **187 Rust tests passed**, including initial and between-match long-lobby
  regressions, the full Running timeout, identity retention and expiry.
- Native clippy passed with the same four baseline warnings; binaries passed.
- Corrected optimized native backend built in **298.174 seconds**. Both actual
  hardened runtime images were rebuilt, and the expanded paired-container smoke
  passed in **8.955 seconds**, including missing/malformed requests, both log
  streams, scoped infrastructure diagnostics, Recreate and SIGTERM.
- The WASM artifact is unchanged: only `src/server.rs`, which is excluded from
  WASM by `cfg`, changed runtime source. The collector verifies all consumed
  client source/assets/Cargo/HTML/modules/build-script bytes against the original
  build revision. Its SHA-256 remains
  `c9eecc0e081e587649fa83b5bec06f82a9730b72890cee37952279d01de57940`.
- `manifest.json` records current source, verification-script, artifact and proof
  hashes and actual local image IDs/revisions. It rejects untested runtime or
  packaging deltas. Images remain local-only until a separately verified release.

## Browser and network acceptance

`production/results.json` records the corrected canonical-origin fixture's real
HTML forms, two concurrent rendered clients, matching authoritative snapshots,
keyboard input, real outage/bot takeover and same-identity reconnect with held
input neutralization. The final canonical run passed in **123.560 seconds** with
**1,135 identical common ticks**. `admission-diagnosis.md` preserves the failed
boundary and measured correction: B received upstream HTTP 101 promptly, but a
shared Chromium/SwiftShader GPU process stalled browser startup. Separate real
Chromium processes remove that artificial contention without changing production
code, deadlines or assertions.

`crossplay/` records **3,594 identical browser/browser/native snapshots** across
19 checks in **548.499 seconds**, including real gameplay, natural victory,
same-lobby return and offline pause. `resilience/` records seven recovery/error
checks in **381.595 seconds**, including **775 identical post-reconnect ticks**,
simultaneous recovery, endpoint/room claim isolation, reload and crash/reset.
Both browser gates recorded zero unexpected errors or server tick overruns.

`rooms-load.json` records one- and sixteen-room timing measurements in
**75.106 seconds**; the minimum sixteen-room rate was **63.997790 Hz**, with
zero observed overruns. These are local-host results, not target-node/WAN
throughput claims. The collector requires successful exit records and
credential-negative scans before packaging.

The canonical local fixture deliberately maps its hostname to loopback and uses
an ignored self-signed fixture certificate. This is **not** Internet reachability,
public certificate trust, live CNI policy, or target-node resource verification.
Native manual graphical testing remains explicitly deferred. Native WSS public
trust and actual public browser/native gameplay are separate rollout checks.

No generated TLS keys or private reclaim credentials are packaged. Raw failed
runs remain in ignored verification storage; representative RED regression logs
and the admission diagnosis are retained here. Registry publication, paired chart
commit-back, Flux rollout and live gameplay must be recorded separately.
