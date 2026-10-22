# Online HUD: local acceptance proof, 2026-10-22

## Scope and claim boundary

The original passing run is `runs/2026-10-22T06-00-41-915Z-8658f8/`.
Its original `results.json.gz` says `passed`, with `errors: []`; its original
`driver.log` records exit 0. This is **local, actual parent-built WASM plus an
owned native loopback backend**, not a public HUD deployment, merge approval,
native graphics test, or manual play session. This package does not change any
run's recorded status. Five earlier actual-verifier attempts remain failed.

The evidence was packaged after execution. New packaging verification records
are explicitly named as such; they are not contemporaneous execution metadata.
No verifier/build/game/browser was rerun to prepare this package. The directory
name is not itself proof of an external deployment. Existing public release 327
is outside this package and is not evidence that this HUD was deployed.

## How to audit

From this directory, run:

```sh
python3 verify.py
sha256sum -c SHA256SUMS
```

To additionally compare the original passing run's source and artifact hashes
to a checkout containing the original built outputs:

```sh
python3 verify.py --repo /path/to/ryggattack-online-hud
```

`manifest.json` records every package file except itself and `SHA256SUMS`, with
byte lengths and SHA-256. `SHA256SUMS` also covers the manifest; it necessarily
excludes itself. `retained-originals.json` maps each copied file to its actual
producer path and records its original, uncompressed bytes and SHA-256.
`verify.py` checks exact membership, every stored hash and size, all decompressed
original hashes, original per-run artifact manifests, historical source archive
manifests, the passing harness's local import closure, and the recorded build
chain. It performs no network calls, builds, browser actions or git mutations.

Large JSON files use deterministic gzip (mtime 0); decompressing yields the
**original bytes**, not a summarized/reformatted report:

```sh
gzip -cd runs/2026-10-22T06-00-41-915Z-8658f8/results.json.gz
```

Original `sha256.json` files intentionally retain their original uncompressed
filenames and hashes. The verifier resolves these to `.gz` where appropriate.
`runs/index.json`, `timings.json`, `pass-source-verification.json` and
`provenance/build-chain-verification.json` are packaging-time indexes/comparisons.
They supplement, never replace, producer reports and logs.

## Actual run, method and timing

Source: the passing run's original report, observation files and driver.
Started `2026-10-22T06:00:41.915Z`; finished `2026-10-22T06:02:27.259Z`.
Report elapsed: **105.344 s**. Outer driver elapsed: **110.396 s**, exit 0.
These are different clock envelopes, not contradictory timings.
Chromium reported `153.0.8010.12`; actual-game viewport was **640 × 480**.
The earlier 1024 × 768 actual run failed freshness; this later smaller viewport
pass does not retroactively establish a 1024 × 768 actual-game pass.

One real WASM human (`HudLocal`) and one bot used the same client, socket and
owned room for both naturally completed cases. There was no forced result,
gameplay patch or restart. Each case retained a separate 180,000 ms natural
completion budget; screenshot captures each had a separate 60,000 ms ceiling.
Capture budgets do not extend match deadlines. The game page closed before
isolated presentation fixtures ran.

| Case-local metric | Real target-seven input/UI preflight | Original target-one natural core |
|---|---:|---:|
| Natural completion duration | 52.013 s | 8.708 s |
| DOM observations | 1,034 | 381 |
| Authoritative frame matches | 856 | 149 |
| DOM matches | 1,020 | 377 |
| Maximum published-frame lag | 64 ticks | 48 ticks |
| Observed sent input packets | 107 | 19 |
| Neutral-fire packets | 94 | 18 |
| Pre-wire-Lobby winner observations | 45 | 45 |
| Winner persistence before wire Lobby | 2,675 ms | 2,675 ms |
| Wire Lobby receipt → actual clear | 461 ms | 587 ms |
| First observed post-Running raw Lobby → actual clear, page clock | 74.90000000596046 ms | 50 ms |

The freshness threshold stayed at 64 ticks. The original actual report contains
case-local first states, zero-win/no-winner resets, winning frames, OUT/IN states,
named outcome observations, return-to-Lobby frames, input windows and timelines.
Preflight focused-form editing and Escape-menu windows each observed three
neutral-fire packets while the server advanced. The core separately exercised
early held Space; preflight evidence is not substituted for core evidence.

The independent page-clock transition oracle allows at most the explicit
100 ms DOM-poll interval after first observed post-Running raw Lobby, requires
serialized-frame and actual roster/outcome/buff DOM clearing, and forbids
post-clear outcome reappearance. It is not a server-send-latency measurement.
The independent wire-receipt-to-clear bound remains 1,000 ms, with named winner
proof before wire Lobby and at least two observations spanning 100 ms.

One sequence-1 input with zero axes and `fire: false`, between the single
Hello/Welcome and initial Lobby, is labeled **startup-neutral only**. It is not
preflight/core input proof. Startup-policy tests and the final focused review
preserve rejection of nonneutral, repeated or late startup inputs.

Cleanup readback confirms Leave sent, room/participant cleared, HUD frame/lobby/
seat cleared, game page closed before fixtures, browser closed, backend exit 0
without signal, and HTTP closed. The pass has no forced-browser-kill entry.

### Actual screenshots versus result evidence

All six original passing screenshots and their producer SHA-256 values remain
in the passing run directory:

| Screenshot | Capture start, run ms | Capture duration, ms | Meaning |
|---|---:|---:|---|
| `preflight-playing.png` | 17,035 | 8,005 | Actual WASM; Running at both brackets |
| `preflight-returned-lobby.png` | 63,528 | 7,771 | Actual returned Lobby at both brackets |
| `actual-playing.png` | 76,784 | 6,856 | Actual core WASM; Running at both brackets |
| `actual-returned-lobby.png` | 85,524 | 6,860 | Actual core returned Lobby at both brackets |
| `fixture-1024.png` | 99,608 | 118 | Isolated synthetic presentation only |
| `fixture-375.png` | 100,008 | 87 | Isolated synthetic presentation only |

Screenshots can span transitions. They do **not** prove an earlier winner;
fast DOM observations and authoritative projections supply that evidence.
The parent reported visually inspecting the latest actual-playing and
actual-returned-lobby images: readable compact two-seat names/wins, diagnostics
opened by the test (not legacy Canvas debug), unobstructed room disclosure,
and no old Lobby roster/outcome/buffs. This records the parent's supplied
inspection, not a new independent visual review by the packager.

## Synthetic scope

The passing actual run separately passed 1024 px and 375 px presentation
fixtures: literal markup-like names, long text, concurrent speed/all-hits timer
labels, OUT states, diagnostics stability, room-control clickability and form
focus, plus synthetic draw/named-bot outcome rendering. These are **synthetic
presentation checks, not natural buff collection or natural buff-timer proof**.
Neither real case observed a natural buff; both original reports say so.

`synthetic-diagnostics/online-hud-synthetic-diagnostic/` preserves the separate
throwaway VM cross-realm extraction's failure, executed probe and failure image.
It failed Node `deepStrictEqual` prototype identity, not a demonstrated product
behavior. Its status remains failed. The same-realm diagnostic has its own
original passing results and two images, plus its original driver log in
`logs/`. Its producer did not retain a separate executed-probe file; none is
invented here. Both identify the archived fixture-source hash. The same-realm
pass is not a full actual-game pass, and does not override actual failures.
`reviews/hud-synthetic-visual-review.json` records an earlier limited synthetic
visual review, including its original caveats.

## Preserved failed actual attempts

Each row has its original results, driver, artifact SHA manifest, observation
files, exact historical executed local sources and source manifest. All
available images in these scoped published run directories are retained;
the first failure occurred before browser launch and has no image.
No failed report was rewritten as a pass.

| Run directory suffix (all 2026-10-22 UTC) | Report elapsed, s | Driver elapsed, s | Original failure |
|---|---:|---:|---|
| `04-43-59-502Z-72de19` | 4.829 | 4.980 | Compared intentionally unshipped Pages redirect with game artifact |
| `04-57-52-865Z-e99f1f` | 39.843 | 44.895 | 1024 × 768 software-rendered HUD exceeded unchanged freshness limit; forced browser shutdown also recorded |
| `05-03-16-744Z-e10765` | 34.082 | 39.141 | Natural target-one game ended before focused input window completed |
| `05-19-34-575Z-f85bd2` | 77.495 | 82.562 | Original oracle incorrectly required all named observations before wire Lobby |
| `05-48-27-252Z-673520` | 99.905 | 104.966 | Exact neutral startup packet was rejected for lacking an active case; overall failed despite later case/fixture successes |

The later pass follows reviewed, scoped harness changes: reachable entry-module
artifact checking; longer real input/UI preflight followed by separate original
target-one core; bounded DOM-poll transition handling; narrowly classified
Welcome-to-initial-Lobby neutral input. Runtime behavior was not changed to
satisfy the later temporal/startup oracles. The product HUD z-index change to 4
was an earlier actual narrow-layout RED/GREEN correction, covered by the
original harness review.

## Native, WASM, Node and compiler provenance

| Gate | Recorded result | Elapsed |
|---|---|---:|
| Whole native suite, `logs/hud-native-final.log` | 201 tests passed across all recorded test groups; zero failed | 171.139 s outer driver |
| Clippy, same log | Exit 0; four warnings at explosions/settings/ui locations, retained verbatim | 4.858 s |
| Native binaries, same log | Exit 0 | 0.543 s |
| Actual WASM build, `logs/hud-wasm-build.log` | Exit 0; wasm-opt reports 53 MB → 39 MB | 208.933 s |
| Final Node, `logs/hud-all-node-startup-fixed.log` | 122 passed, zero failed/skipped | 2,168.643694 ms TAP duration |
| Throwaway cross-realm diagnostic | Exit 1, separately classified above | 0.512 s |
| Separate same-realm presentation diagnostic | Exit 0, synthetic only | 1.111 s |

`timings.json` extracts every explicit TIMING/TAP duration line from retained
logs and run drivers, all case durations, run envelopes and screenshot timings.
Missing timings are not inferred; these timing envelopes must not be added as
if they were independent work.

The native count is the sum of the original native log's passed test groups,
not just its 174-test library group. Test execution does not establish native
renderer/display behavior. Four Clippy warnings remain; this is not a
warning-free claim.

`provenance/hud-runtime-source-before-build.json` records the runtime source
state before the actual WASM build; `provenance/hud-built-artifact.json` records
the unchanged source state and built output hashes. The recorded base HEAD is
`1b0fd6a471ff9f470f9262db6310df88312b0164`, **not** a claim that the dirty HUD
changes were already committed there. Per-file content hashes establish the
source state.

The retained WASM metadata is 39,799,188 bytes, SHA-256
`616a8724009a7d548190a50dfdfc205cb9e57a7c001b24d29ec5f32d80fea8a1`.
The build-chain comparison verifies pre-build state equals post-build source
state; those sources still match disk; built outputs equal the actual passing
run's recorded artifacts. No WASM/native executable or asset binary is copied.
This is provenance from actual producer records and comparison, not a claim of
an independent reproducible rebuild.

## Commit equivalence and completed local gates

The reviewed implementation was committed as
`a2c90aa411d0104d5459398ecceaf1c4cfb336f0`. A normal merge of upstream
`520a47b8252e9ea8a6264e77dc69f00b990713bd` produced
`59c0c47713c608a5aa41f3470b723f9d07268dfc`; the only change from the
implementation commit was the already-deployed chart appVersion. Parent
readback rechecked all 104 execution hashes and all six screenshot hashes.
`provenance/hud-main-base-merge-equivalence.json` records this comparison;
it does not relabel the earlier dirty build as a build of a later Git HEAD.
Strict Helm/chart checks passed again after that chart-only merge.

The completed extra local gate driver is retained as
`logs/hud-final-extra-gates.log`, with separately observed exit 0 in
`provenance/extra-gates-parent-readback.json`. It exercised native TLS,
server banner/policy, production/socket smoke, the actual headless executable,
Helm/chart and chart-version contracts. TLS keys were generated only in owned
scratch and were not packaged. These are local gates, not remote CI or rollout.

## Source identity and review boundaries

`pass-source-verification.json` verifies **all 104** source/artifact records
against current disk, including binaries by streaming SHA-256 without retaining
them. `source-at-pass/` archives matching text source: Cargo manifests/lock,
recorded scripts/tests, Rust sources and web source. This includes the new
startup helper/tests even though they were not yet indexed during packaging;
their bytes are independently named in the original pass hashes. The exact
main harness and six recursively imported local helpers are included. Node
built-ins and installed package implementations (Playwright/Acorn) are not a
vendored dependency environment; this is not an archive of node_modules.

Historical failed-run source manifests are checked against their historical
archives, never silently replaced with final sources. The passing source
archive was created during packaging **only after** matching the original
execution's recorded SHA/length; it is not falsely labeled a run-created file.

All eight original review records remain under `reviews/`. They are static or
limited visual reviews, not acceptance executions. The original code/harness,
packaging and orchestration approvals retain their scope and explicit failure
caveats. `hud-temporal-review-blocked.json` remains blocked; its correction is
covered by `hud-temporal-review-final.json`. Startup-policy approval is in
`hud-startup-review-final.json`. Their original “pending separate execution”
strings are unchanged; the later original actual passing run is the execution
evidence. Partial approvals never override failed runs. Scoped packaging and
startup TDD artifacts are retained under `tdd/`.

## Privacy, retention and whitespace

This package used an explicit allowlist of already-published scoped proof,
producer logs/reviews and matching known project source. It did not copy an
arbitrary verification tree, credentials, environment files, browser profiles,
TLS material or scratch certificates. In particular `verification/native-tls`
is not retained. No private claim-token payload is intentionally retained.

The actual report field is **`private_material_negative_scan`**, not
`privacy_scan`. The passing report records `passed: true`, one known value
checked, exact-value scanning of all artifacts, and visible-DOM checks before
screenshots. This is the producer's limited negative scan, not a general
absence-of-secrets guarantee, OCR audit, or new packager access to secrets.
The throwaway diagnostics explicitly say no credential and are separately
classified; no privacy metadata was fabricated for them.

Immutable retained logs and historical executed-source evidence may contain
original trailing spaces or blank EOF lines. `whitespace-findings.json` lists
actual `git diff --no-index --check` findings. **Any exception applies only to
immutable copied evidence, to preserve byte hashes**, not to edited product
code, new package documentation, manifest tooling, or future changes. No global
whitespace exemption or repository policy change is made. Do not “clean” an
original log/source archive and then continue citing its old hash.

## Still unchecked / external gates

- [ ] Native graphics/display/manual session acceptance.
- [ ] Remaining explosion-scale visual acceptance.
- [ ] Remaining props/arena visual acceptance.
- [ ] New HUD merge, paired immutable publication/deployment and independent
      external public HUD proof. This local package makes none of those claims.
- [x] Additional local native-TLS/banner/policy/production/socket/headless/
      Helm/chart gates: completed, parent observed driver exit 0; original log
      and explicit parent-readback metadata retained. Remote PR CI is separate.

To add later independently verified proof, retain original producer bytes and
update its provenance/retention index explicitly; do not alter prior statuses.
After a deliberate package update, run `python3 verify.py --refresh-manifest`
then `python3 verify.py` and `sha256sum -c SHA256SUMS`. Refreshing hashes is not
an acceptance execution and must never be used to conceal modified originals.
