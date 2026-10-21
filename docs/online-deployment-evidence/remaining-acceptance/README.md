# Remaining web-first acceptance evidence

Runtime under test: published `sha-205d048` web/backend pair. This branch adds tests, verification tools and documentation, not gameplay changes or a deployment. Native graphical interaction remains user-deferred. Presentation/art parity is distinct from gameplay-entry parity and is not certified by CPU-side tests.

`manifest.json` binds all archived proof files (except itself) and the tested source files to SHA-256 digests. Raw log bytes are preserved, including TAP whitespace; source whitespace checks exclude only these immutable evidence logs, not application or verifier code.

## Public solo lifecycle: passed with one bot

`solo-onebot/results.json` records the actual public canonical HTTPS/WSS run on 21 October 2026. One human used the real room form and keyboard, configured Forest / one bot / first to one, and traversed Ready → Countdown → Running → a natural winner → the same lobby. No outcome, state, protocol packet, score or deadline was injected or overridden.

- Natural completion and lobby return: **8,750 ms** from the completion-budget start; whole process **31.627 s, exit 0**.
- 140 authoritative frames, 47 finished-round/elimination frames, 18 actual firing input packets. Steering was neutral with held fire, matching the characterized ordinary input policy for this one-bot fixture.
- Same room, participant and settings retained, readiness cleared. Actual outgoing Leave, visible Left-room status and browser closure verified.
- The two original PNGs were visually reviewed: actual Running gameplay and the returned lobby with Forest / one bot / target one / host not ready.
- Known private reconnect value was excluded and negative-scanned. Executed verifier source and original result/screenshot hashes are archived.

### Preserved three-bot failure — not converted into a pass

`solo-threebot-failed/` retains the original public one-human/three-bot run: Forest, seed 30, target one, held Left and Fire, no winner within the unchanged 180-second completion bound. It recorded 3,183 frames and two eliminations, so starting/progressing alone was not accepted as completion. The original executed verifier is preserved byte-for-byte.

Separate, extended **local-only** headless characterization observed legitimate first points at approximately 340 simulated seconds for seed 30 and 386 for seed 2. Those diagnostic runs do not replace the failed public attempt, prove a public three-bot completion, or establish a bounded completion guarantee for every roster. No AI, collision, combat, scoring or production time limit was changed to satisfy the fixture. The one-bot pass establishes the basic solo online match lifecycle, not universal bot-endgame liveness.

## Visible client errors

`feedback-initial-guard-failed/` preserves the initial setup failure: Podman expands `--cap-drop=ALL` into individual capability names, so the verifier's Docker-specific inspect-string assertion failed before any browser check. The corrected verifier keeps `--cap-drop=ALL` and additionally requires actual kernel effective, bounding and ambient capability sets to be zero; it does not weaken container hardening.

`feedback-prior-pass-runner-incomplete/` preserves the subsequent real browser report. All four terminal cases had visible DOM notices, enabled Create/Join recovery controls and visually reviewed screenshots:

1. Graceful restart: `backend restarting; rooms reset; join a lobby again`.
2. Formerly valid invitation rejected after same-image backend recreation: `room not found`.
3. Fifth real client rejected from a four-human room: `room full`.
4. Real admitted transport interrupted by the owned proxy: bounded retries followed by the unavailable/reconnect-expired message.

The saved report finished in 188.454 seconds with browser/container/TLS cleanup and a seven-value private-material negative scan. However, the supervising systemd service exited **120**, and its timing wrapper did not record successful completion. These saved functional results are **not** represented as a clean process pass.

A subsequent clean repeat is archived in `feedback-clean/`: all eight observations including the four terminal cases passed, the complete supervising job exited **0**, and the timing wrapper recorded **191.333 s, exit 0**. Hashes and exact executed source were independently checked; owned browser/container/TLS cleanup and the seven-value negative scan passed. This clean run exercised the archived verifier, not the later Docker-endpoint/privacy hardening.

The final hardened verifier was then exercised separately against the same published pair: `feedback-hardened/` passed all eight observations in **189.672 s, exit 0**, including verified cleanup and the seven-value private-material scan. Its main-source hash matches the executed file; imported helper bytes and hashes are archived alongside it. The fresh harness review approved the changes with no security/logic findings (`reviews/harness-final.json`), following the preserved initial blocked review. Docker endpoints are validated before output or daemon effects and explicitly pinned; the real end-to-end fixture used non-remote rootless **Podman**, while Docker selection/pinning has unit and inert-process coverage, not a claimed full Docker-engine replay.

The maintained solo verifier subsequently gained DOM-before-capture privacy checks, all-artifact publication scans/purge, observed roster-policy assertions and inert CLI guards. Its hardened helper behavior is unit-tested; it was **not** represented as the verifier used by the earlier public one-bot pass. No additional public room was created for those guard changes. Exact-byte scans over compressed screenshots are supplemental and are not OCR or a proof about all rendered text; the real captures were visually inspected for the acceptance cases. The archived executed-verifier files are provenance snapshots, not standalone entry points: use the maintained scripts at the repository root.

These fixtures use isolated rootless copies of the exact published images, with the canonical origin mapped only to loopback. Before any game command, the real navigation must return the run-specific local-fixture header. No public backend is restarted. TLS verification is relaxed only for that local self-signed fixture; this is not public TLS acceptance.

## CI portability correction

The initial PR head `39de1b7` failed CI run324/UI54 before Cargo ran: the Node privacy test assumed `/root/.hermes/cache/scratch` already existed when `TMPDIR` was unset. Helm and web-build jobs passed, but the overall required gate remained failed. The original failure is recorded in `ci-initial-failure.json`.

The test now creates its scratch parent before `mkdtemp`. A subprocess regression exercises a fresh HOME fallback and an explicitly absent TMPDIR, requires an actual one-test/one-pass child run and checks that temporary test artifacts are removed. It clears the inherited nested-runner marker so a skipped inner file cannot masquerade as a successful test. The initial missing-parent RED, nested-regression RED and final GREEN logs are preserved. The corrected full 34-test Node matrix also passed with a fresh HOME and no TMPDIR. This changes only test setup; previous Rust and browser artifacts remain valid for their recorded source. The corrected commit still requires fresh exact-head CI.

## Gameplay and reconnect regressions

Nine CPU-side entry-parity tests compare offline scene/Playing/input/fixed-loop entry against actual Room countdown and framed, serialized input handling. They cover both arenas, seeded topology and pickup kinds/positions, actual AI, realized junction selection, front-only rear-contact push, projectile direction and AllHits ownership, Speed's 1.25× movement effect, collection/expiry, elimination, scoring and round reset. Canonical comparisons also include live scenery colliders, projectile owner/velocity/lifetime, and pending detonation position/chain/fuse. Real prop strikes exercise projectile consumption, delayed chain destruction and exact forest restoration after an automatic round reset. Requested steering permits only the documented last-bit normalization tolerance of less than `1e-6`. This is not a GPU/presentation test. The offline fixture manually registers the gameplay-relevant client systems instead of invoking the graphical `run_client` bootstrap; future client registration changes must be reflected in this fixture. The independent Rust review passed with no security or logic findings (`reviews/rust.json`); its optional stronger neutralization/transient-boundary assertions are not claimed as already implemented.

A separate deterministic running-room regression queues actual old-socket Input and optionally Leave frames before reclaim, proves complete frame arrival, reclaims without moving/respawning the cart, and checks that the replacement's low sequence numbers and real inputs alone control the cart. Existing real-admission, incorrect/cross-room/rotated/replayed and exact-boundary expiry tests remain required. A currently valid stolen bearer can replace its owner: the user explicitly accepted and documented that v1 limitation.

The expanded exact-source local Rust suite passed **197 tests** in **169.882 s** (test compilation approximately **2m11s**); Clippy passed in **3.269 s**, with only the four pre-existing warnings and no new warnings. Native build, real TLS, server policy/abuse, socket and headless gates passed in the preceding sequential run; only `cfg(test)` source was subsequently extended. The clean error replay's terminal screenshots were visually reviewed. Final helper tests passed **18/18**; the full post-fix non-build matrix passed **34 Node tests**, room UI, Pages, portable builder, chart-version and Helm checks. Both final independent source reviews approved their respective Rust and harness scope. Exact-head Forgejo CI remains a separate PR gate; consult the PR checks rather than treating local verification as CI or a deployment. Manual native graphics and presentation/art parity remain outside this evidence, and this PR does not automatically close issue #6.
