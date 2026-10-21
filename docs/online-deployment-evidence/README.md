# Phase 5 local deployment verification

**No registry push, remote Git/issue write, cluster change or public deployment was performed.**
These are real local process, image, browser and Helm results, not rollout proof.

## Provenance

- App/WASM build source: `b72055fa779908e5450c80b367841b1b78575b9e`.
- Final implementation/image source: `7ce74a3da89eff726e81a2417b271726b7add98a`.
- All consumed Rust/Cargo/assets/HTML/browser-module/build-script inputs are
  byte-identical between those revisions and the final worktree. Subsequent
  changes fixed image build residue/base-library handling and verification/CI;
  they did not change the tested game bundle. Images were rebuilt and exercised
  after the image changes. The final evidence commit adds documentation and the
  corrected same-image Recreate harness only.
- `manifest.json` records input hashes, JS/WASM/native binary hashes, image-source
  metadata and every collected proof-file hash. `phase5-images.json` records the
  actual **local-only** Linux/amd64 images and their OCI revision labels/digests.
- The exact optimized WASM tested in every browser run and served byte-for-byte
  by the final nginx image has SHA256:

  `c9eecc0e081e587649fa83b5bec06f82a9730b72890cee37952279d01de57940`

- The release backend executable inside the image equals the host-built
  executable, SHA256:

  `b3153a6fc603a2e81018f0b24934f8c10837dcbf99f42aab420209d6d4befbb3`

The toolchain versions are recorded in `phase5-toolchain.json`. These hashes
identify the tested local artifacts; they do not promise bit-identical output
from a later CI toolchain/base-image rebuild.

## Completed final gates

| Actual command / gate | Result | Wall time |
| --- | --- | ---: |
| `cargo test --locked --config 'profile.dev.package.ryggattack.strip="debuginfo"'` | **185 Rust tests passed** | 32.044s final warm run; 183.333s initial final-source compile/run |
| `cargo clippy --locked --all-targets` with the same package-only strip override | PASS; four warnings, exactly the prior-main warning kinds/count | 0.393s final warm run; 12.023s initial run |
| `cargo build --locked --bins` with the same override | PASS, including shared native adapter/client | 0.359s final run |
| `scripts/build-web.sh` | PASS, including wasm-bindgen and wasm-opt 131; 53MB → 39MB | 501.800s |
| `cargo build --locked --release --bin ryggattack-server` | PASS, single cargo job; ordinary release profile | 1262.342s |
| Final backend / paired web image builds | PASS, rootless Podman | 1.117s / 0.697s (cached web layers); earlier fixed web build 3.975s |
| `ENGINE=podman node scripts/container-smoke.mjs …/web:sha-7ce74a3 …/server:sha-7ce74a3` | PASS, actual hardened images and sockets | 9.014s |
| `node scripts/production-smoke.mjs` | PASS, active simulation, strict origins/path, private probes/metrics, creation/guess limits, spoofed forwarded headers, SIGTERM | 5.090s |
| `node scripts/network-sockets.mjs` | **16 abuse checks passed**, including empty non-FIN trickles/floods while healthy ticks advance | 10.367s |
| `node scripts/native-tls.test.mjs` | Real native ClientHello/certificate exchange; untrusted certificate rejected, no plaintext downgrade | 2.108s |
| `python3 scripts/headless-smoke.py target/debug/ryggattack-headless` | Actual no-display executable, 2048/8192 ticks, movement/fire/pickups/scenery/solo rules/real match and round restoration | 1.163s |
| `node --test scripts/refresh-rate.test.mjs` | 10 passed | 0.137s |
| Room UI / harness / Pages redirect / chart-version updater / fail-closed server policy | All PASS | Individual commands/times in `timings.tsv` |
| `python3 scripts/chart-test.py` (PyYAML) | Real Helm render plus resource, origin, route/privacy, immutable pair, main-only publication/skip-ci/Pages contracts | 0.192s |
| `helm lint charts/ryggattack --strict` | PASS; only optional icon recommendation | 0.039s |

Heavy native/WASM compilation was serialized. Local dev/test binaries used only
an explicit **package-level debug-section stripping** override to avoid disk
exhaustion; optimization levels and shared dependency flags were not changed.
Production native and WASM artifacts used the checked-in release profile.
`RUSTFLAGS`/`LIBRARY_PATH` were reset before sourcing `rust-dev-env.sh`.
`phase5-runtime-linkage.txt` confirms the actual release binary's libc/libm/libgcc
runtime linkage; this remains a CPU-side Bevy-Assets service, not an asset-free
simulation crate.

## Real rendered browser results

No compiler, linker, wasm-bindgen or optimizer ran during rendered verification
or load measurement. Screenshots were inspected, not merely created.

- **Canonical HTTPS/WSS: 95.106s**, `production/results.json`.
  Two actual Bevy WASM clients used the real create/join forms and their computed
  default `wss://ryggattack.modig.online/ws`. The backend observed the exact
  `https://ryggattack.modig.online` Origin on `/ws`. Configure/ready/start,
  keyboard steering/fire, identical authoritative snapshots, a real socket cut,
  bot takeover and same-private-identity/seat reconnect all passed. Held input
  remained neutral on return. No unexpected browser errors.
- **Browser/browser/native crossplay + offline: 402.859s**, `crossplay/results.json`.
  Distinct seats 0/1/2, actual graphical WASM clients plus the shared native
  adapter, real input and 75ms each-way TCP delay. **2,588 common authoritative
  snapshots matched** at shared ticks. All clients observed movement,
  projectiles, eliminations, scoring and natural victory/return to the same
  lobby. Native median acknowledgement RTT was 188.351ms with that injected
  delay. Server clocks measured about 64Hz, zero overruns, no browser errors.
  HTML form focus, online Escape and real canvas blur suppressed input without
  pausing the server. Offline launch/play and the visible **PAUSED** overlay
  passed on the exact same artifact, without opening a socket.
- **Reconnect/resilience: 199.020s**, `resilience/results.json`.
  Two rendered clients recovered, the last-human drop froze simulation across a
  real wall gap, private saved-seat reload and endpoint/room claim isolation
  passed, and held controls required release before fresh input. An actual
  SIGKILL/restart exercised lost-memory stale-claim rejection (SIGTERM is now
  graceful). Missing/full/incompatible/unavailable states and deliberate Leave
  cleared claims as appropriate. Zero unexpected browser errors/overruns;
  seven expected transport console errors during deliberate outages.

The canonical HTTPS fixture mapped the hostname **only to loopback/high port**
using Chromium host-resolver rules and a generated ignored self-signed
certificate. Its explicit local certificate bypass is **not trusted public TLS
or a cluster test**. Native WSS certificate rejection was tested without a trust
bypass. Native graphical/manual interaction remains explicitly deferred; only
the shared native adapter and native/headless binaries were exercised.

Online visuals remain the prior prototype/proxy presentation with a large debug
HUD; this work does not claim a graphics redesign or polished low-resolution UI.
The final `BrowserA.png` captures the returned lobby; the
`initial-playing-BrowserA.png` / `initial-playing-BrowserB.png` captures show real
online carts/rails. Offline gameplay/pause screenshots show the ordinary game.

## Actual image and load checks

Final paired local images use `sha-7ce74a3`; neither was pushed:

- web config/image ID `2ad188584f1ab181c25686aabf2468d330ca772aed6028a4bdcb40c7d98eed5e`;
- server config/image ID `ea647fc31db0d7632faeac55d80ba986b2ea7cd79b9028298991970ff18d4c55`.

The real image smoke verified nonroot UID/GID 101 / 10001, read-only roots,
dropped capabilities/no-new-privileges, exact served HTML/JS/WASM/module hashes
and MIME types, private-query-free web access logs, public-web 404s for management
paths, strict production socket policy and abuse limits. Recreating the backend
from the **same image** produced zero rooms; an active real game client then
received the reset error and closed on SIGTERM, with process exit 0 and the
asserted stop bound under two seconds. All task test containers were removed.

`rooms-load.json` records 20-second steady windows after actual room starts:

- 1 room / 1 human / 3 bots: minimum **64.000073Hz**, no overruns;
- 16 rooms / 16 humans / 48 bots: minimum **64.000900Hz**, no overruns.

The complete load gate took 75.159s. These are local host results with no
compiler/GPU contention, not a guarantee for the target node/WAN or all 64 human
connections. Target-node resources and private tick metrics still need rollout
validation.

## Observed failures and corrections (preserved, not counted as passes)

- Behavior-first RED logs show missing production binding, insecure browser
  default, unrestricted anonymous guessing and the missing silent-running-peer
  timeout. Corresponding GREEN gates passed.
- Native TLS was initially absent; then enabling TLS exposed a missing Rustls
  crypto provider. The final real-wire test verifies ClientHello/certificate
  exchange and rejection. An early fixture expected a flushed TLS alert; it was
  corrected to observe the actual handshake/certificate instead of relying on
  alert-delivery behavior.
- Two early build batches were **deliberately SIGTERM-interrupted**: first for
  a full filesystem while linking debug ELFs, second after successful recovery
  gates when the missing native TLS dependency was discovered before release.
  They are not successful final gates and have no completed timing summary.
  Owned children were checked/stopped, logs retained, and final gates rerun.
  Historical cgroup `oom_kill=2` did not increase; neither termination was claimed
  to be OOM. The subsequent package-only strip override avoided repeating the
  problem without changing game optimization or deleting dependency caches.
- The first crossplay run ended at a fixed-sleep form check after its genuine
  first-to-one match had returned to Lobby. Inputs were neutral, errors and
  overruns zero. `round-boundary-fixture-failure.json` preserves the diagnostic.
  The corrected harness requires three actual neutral packets and only retries
  real round boundaries; any non-neutral packet still fails, including across
  a boundary. Its unit tests and full corrected rendered run passed.
- A real nonroot image startup found root-owned `/tmp/nginx.pid` left by
  build-time `nginx -t`. Removing that residue fixed the actual image. Early
  smoke URL construction also now rejects absent/non-loopback published ports
  before fetching, rather than normalizing an empty mapping into another host.
- The redundant Debian apt install failed under this nested rootless mapping's
  `_apt` group transition. Actual base `dpkg-query`/linker-cache reads proved
  libgcc-s1 already present; the Dockerfile now asserts it without a network
  install or weakened runtime privileges. The final backend image passed.
- Podman/pasta rapid same-port restart hit a host-port teardown race. The harness
  now exercises the actual chart **Recreate** contract with a fresh namespace
  and rediscovers ports, separately retaining the active-client SIGTERM check.
  The complete final image gate passed; no application error was ignored.

All completed build/test timings, including expected REDs and superseded runs,
are retained in `timings.tsv` and `logs/`. Raw originals and the ignored progress
checkpoint remain in the worktree's `verification/` directory. No private reclaim
credentials or generated TLS private keys are included in this evidence.

## Remaining deployment boundary

The chart fixes one replica/Recreate and all room data is in memory. Every
backend replacement resets rooms. Admission quotas are intentionally global,
not spoofable forwarded-IP buckets; anonymous load can still deny new joins or
reconnects. Metrics are unexposed by Ingress and namespace-private by policy,
but live CNI enforcement must be checked. Registry/Forgejo workflow permissions,
paired publication, chart commit-back, Flux readiness, public trusted TLS/WSS and
live resource behavior are the parent's separately authorized rollout gates.
The initial chart tag is an unpublished backend bootstrap placeholder: wait for
both main-release images and the `[skip ci]` appVersion commit before bootstrap.
