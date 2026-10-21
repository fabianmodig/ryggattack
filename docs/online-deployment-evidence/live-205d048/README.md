# Canonical deployment acceptance — 21 October 2026

The deployed game at **https://ryggattack.modig.online/** passed the corrected
live verifier. This is actual trusted HTTPS/WSS acceptance, not a local TLS
mapping or evidence that the original 15-second screenshot harness passed.

## Frozen application and result

- Runtime pair: `sha-205d048`, full source
  `205d0482790bda115c6d907ce6ff52dfc472a56c`.
- Reviewed routing deployment: PR #14, main
  `12b6f60cd71a86e6aba519af51b1131f0d60d0c7`. No runtime rebuild or restart
  was needed for the final evidence/documentation work.
- Actual successful command, from the base checkout with existing native probe
  and Playwright package resolution configured:

  ```sh
  DEBUG=pw:protocol node verification/live-deployment-measured-capture.mjs \
    --live --out verification/live-deployment/<fresh-run>
  ```

  `measured-verifier.mjs` archives the exact executed source. Copy it to
  `verification/live-deployment-measured-capture.mjs` to preserve its relative
  helper import before reproducing. The harness requires a fresh output path
  and creates only its own disposable private room. Chromium uses SwiftShader,
  one process per rendered participant. It neither builds nor starts servers,
  changes DNS, nor disables public certificate validation.
- Timed wrapper: **194.177 seconds, exit 0**. Full result and protocol metadata
  are under `passed/`; JS/WASM/HTML/module hashes are in `passed/results.json`.
  The resumed readback fetched those same public bytes and rechecked Ready
  source/Helm/certificate and both unchanged pod UIDs/digests/zero restarts.
- Two genuinely rendered WASM clients plus the real shared native adapter:
  distinct seats, actual ready/countdown/Running, real steering/fire packets,
  actual connection outage, bot takeover while the peer continues ticking,
  same participant/seat reclaim, and three neutral packets before held-key
  release. **1,666 common authoritative snapshots** compared equal inside the
  verifier, including **eight common snapshots after reconnect**.
- The retained result contains 16 sampled snapshot hashes, not the complete
  in-memory frame streams. Independent resume validation checks result
  invariants, samples/ranges, identity equality and clock arithmetic; it does
  not pretend to recompute all 1,666 comparisons from those 16 samples.
- Active three-client clock measurement: **63.948 Hz**, near the intended
  64 Hz. No unexpected browser/protocol/native errors; native stderr zero
  bytes. Privacy scan checked three actual reconnect-secret values. Packaged
  proof bytes were additionally scanned against local operational credentials
  without retaining or printing them.
- Both PNGs were manually viewed: real Forest rail arena, three participants,
  Running and Online status; recovery retains seat 1 and transfers host to
  LiveB. The large prototype HUD is not polished. Browser HUD acknowledgement
  RTT is around 1.8–2.1 seconds under software rendering: this is correctness
  evidence, **not a claim of low-latency interactive performance**.

## Screenshot-only verifier correction

Original attempts are preserved under `failed/`: initial ingress failure,
post-route screenshot failure, 640×480 diagnostic, original protocol trace,
and the process-local `fromSurface=false` experiment. None is relabelled as
passing. Original 15-second capture failed even though Chrome eventually
returned genuine captures after **31.9–46.6 seconds**. Layout/preparation was
milliseconds; changing viewport or surface mode did not resolve the cause.

The successful candidate keeps the original 800×600 viewport, capture pipeline,
all gameplay/recovery/snapshot assertions, trusted TLS/Origin checks and every
production deadline. Only the screenshot artifact allowance changes from
15,000 to **60,000 ms**. Its additional diagnostic instrumentation retains only
allowlisted protocol method/ID/label/timing/error metadata and drops raw
protocol payloads before writing stderr. Installed Playwright files were not
modified. Actual successful capture calls took 34.109 and 41.630 seconds.

The independent completed review (`independent-review.json`, source session
`20261021_120033_8497c2`, message 23876) approved this measured correction with
no security or logic blockers. It did not certify the application by static
review. The unchanged 420,000 ms overall check remains **cooperative**, not a
hard process-wide watchdog. The diagnostic stderr filter drops unknown
messages; structured results, explicit assertions and exit status establish
success, not quiet stderr.

The maintained `scripts/live-deployment-smoke.mjs` adopts only the same 60s
screenshot allowance. It does not adopt the diagnostic stderr filter. The
successful run used the archived instrumented candidate, not that later
maintained file; functional equivalence is limited to the documented delta.

## Operations and limits

`operations/` retains actual registry/OCI revisions, publication and exact-head
CI, canonical trusted WSS 101 / bad-Origin 403 / subpath 404, public-IP hairpin,
certificate, hardening/resources, CNI policy and log/clock evidence. Both
immutable image manifests and their revision labels were independently checked.
Release CI exercised fresh compiler output against both actual images; that
publication proof is separate from this resumed serving-byte comparison.

Management 8788 was actually allowed within its namespace and denied from an
unrelated namespace, with game-port WSS upgrade as positive control. It is not
publicly exposed. Owned diagnostic pods/Ingresses were removed and absence
verified in the original operational proofs. Backend rooms remain in memory;
a restart or Recreate loses rooms. No persistence, seamless upgrade, or capacity
SLA is claimed.

One earlier **cumulative tick overrun** exists. The retained idle 10-second
sample measured 63.941 Hz with no new overrun, but zero connected clients; do
not present that idle sample as an active-load benchmark or lifetime zero.

Both browsers sent Leave and peer-observed removal. Native probe termination
was controlled SIGTERM and its process closed. Its final reserved seat/room
expiry was **not directly verified server-side** by this live run. No claim
of immediate server-side deletion follows from local process cleanup.

Public-IP trusted HTTPS/WSS hairpin was verified from the LAN, not from an
independent off-LAN network. Manual native graphical-window testing remains
explicitly deferred; the shared native network adapter was actually exercised.
The parent issue remains open for acceptance wording/scope not fully exercised,
including complete solo-bot match and visual/gameplay parity sign-off. Do not
blanket-check the parent acceptance list from this deployment pass.

## Integrity

`manifest.json` lists every retained proof by relative path and SHA-256. All
references must be tracked in Git, including logs that repository ignore rules
would otherwise omit. Failed original evidence also remains in the ignored
base-checkout verification directories; this package keeps their scrubbed
results and available metadata rather than replacing them with a success.
