# Production deployment (phase 5)

Canonical game: **https://ryggattack.modig.online/**. Browser transport:
`wss://ryggattack.modig.online/ws`. This change supplies application/runtime,
images, chart and local evidence, **not a live deployment**. Infrastructure,
DNS, registry credentials, public certificate verification and rollout belong to
the separately reviewed infrastructure change.

## Runtime contract

Two images share one source tag (`sha-<commit>`):

- `git.modig.online/fabianmodig/ryggattack/web`: nginx, UID/GID 101, TCP 8080.
- `git.modig.online/fabianmodig/ryggattack/server`: native CPU-side Bevy service,
  UID/GID 10001, WebSocket TCP 8787; private management TCP 8788.

The server still uses the shared gameplay and CPU Bevy `Assets`/visual state;
it is **not an asset-free or separately extracted simulation binary**. It does
not start a window or GPU. Gameplay, rear-contact collision rules and offline
pause remain in the shared simulation/client, not in deployment code.

`RYGGATTACK_MODE=production` is an explicit opt-in to non-loopback binding.
Production also requires `RYGGATTACK_ALLOWED_ORIGINS` to be exactly
`https://ryggattack.modig.online` (no wildcard, path, trailing slash, explicit
443, list, HTTP, `null` or silent default). Only the exact request URI `/ws`
upgrades in production, not `/`, `/ws/`, query suffixes or management paths.
Browser Origin must occur exactly once and match exactly. **Absent Origin is
allowed for native clients by design**; Origin is not authentication and a
custom client can forge it. There are no accounts, passwords or public room
listing. Possession of a room code/link grants admission, not a reserved identity.
Secure, rotated private reconnect credentials remain distinct from invitations.

Native clients use the same adapter with Rustls/ring and WebPKI roots for WSS.
After the operator confirms public TLS, connect with
`cargo run --bin ryggattack -- --online wss://ryggattack.modig.online/ws --name Ada`
(and `--room <code>` to join).
A local real TLS test confirms TLS negotiation and rejection of an untrusted
certificate. It does not establish a trusted connection to the not-yet-deployed
public certificate, nor exercise a native graphical window.

Local development is unchanged:

```sh
cargo run --bin ryggattack-server -- 127.0.0.1:8787
# defaults: development, loopback only, browser origins localhost/127.0.0.1:8000
python3 -m http.server 8000 --directory dist/web
```

An HTTP loopback page prefills `ws://127.0.0.1:8787`; HTTPS/non-loopback pages
prefill secure `wss://<location.host>/ws`. Production pages must use HTTPS;
HTTP Origin is intentionally not allowed by the backend. Explicit `online=`
URLs still override it for development harnesses. Loading the ordinary page
without `online=` remains offline; merely opening the room controls does not
create a connection. Real create/join forms supply the computed endpoint.

Direct production process (TLS terminates at Traefik, **not** in the binary):

```sh
RYGGATTACK_MODE=production \
RYGGATTACK_ALLOWED_ORIGINS=https://ryggattack.modig.online \
RYGGATTACK_MANAGEMENT_BIND=0.0.0.0:8788 \
./ryggattack-server 0.0.0.0:8787
```

Do not publish the plaintext WS or management host ports to the internet.

## Bounded service and abuse policy

One process, one replica, **16 in-memory rooms**, four participants including
bots in each room; no persistence or cross-pod room directory. The chart fixes
`replicas: 1` and `strategy: Recreate` so a rolling overlap cannot split rooms.
No PVC is created. Capacity is not a 16-room performance/SLA guarantee; see local
load evidence and validate the chosen CPU limit on the target node.

All admission limits are **global per process**, not per source IP:

| Work | Bound |
| --- | --- |
| TCP accept attempts admitted to workers | 128 per fixed 10-second window |
| Anonymous room creation attempts | 16 per fixed 10-second window |
| Join/code guesses and reconnect attempts combined | 64 per fixed 10-second window |
| HTTP/Hello workers / validated admission queue | 8 / 4 |
| Accepted sockets examined each tick | 8 |
| HTTP upgrade / initial Hello absolute deadline | 1 second / 10 seconds |
| Input message/frame size | shared protocol bound (1 KiB) |
| Raw input read work per peer per tick | 1 KiB and four read calls |
| Completed input/control messages per peer per tick | 8 |
| Admitted-peer partial/fragmented message absolute lifetime | 1 second (initial Hello uses its separate 10s admission bound) |
| Running peer with no valid input | 120 seconds, then ordinary disconnect/reservation |
| Missing live input | neutral intent after 1 second |
| Empty room / reconnect identity retention | 60 wall-clock seconds |
| Lobby inactivity | 30 minutes, reset on meaningful activity/return from match |

Fixed windows can permit adjacent-window bursts. Valid reconnects share the
join budget; a sustained anonymous attacker can deny new admission globally.
Limits bound work/state, not fairness or volumetric DDoS. **Neither Forwarded
nor X-Forwarded-For is consulted**, and there is no implicit trusted-proxy policy.
Source IP behind Traefik is not reliable for per-user quotas. Adding per-IP
fairness requires an explicit trusted-proxy/address policy and tests, not blindly
trusting forwarded headers. Ingress/network-level rate/connection protection is
an additional operator responsibility. Browser WebSocket allocation and native
frame handling still trust the selected backend, as documented in prior phases.

## Health, telemetry and shutdown

Only port 8788 serves management HTTP:

- `GET /readyz`, `GET /healthz`: 200 only after a completed authoritative tick,
  with that heartbeat less than two seconds old and shutdown not requested.
  Otherwise 503. This detects a stalled game loop, not just a listening socket.
- `GET /metrics`: Prometheus-compatible aggregate numbers, without identifying
  labels: rooms, human players (including reserved seats), connected players,
  pending admission workers, completed server ticks, discarded-debt tick overruns and
  admission-rate rejections. This is not per-room gameplay tracing.
- Other paths: 404. The listener has a fixed 1 KiB request buffer, 100ms absolute
  read/write bounds, one request at a time and no keep-alive or worker explosion.

Management is ClusterIP-only, absent from Ingress, with a chart NetworkPolicy
allowing port 8788 only from same-namespace pods (kubelet probes use node traffic).
A monitor in another namespace needs a separately reviewed explicit policy.
CNI enforcement must be verified during rollout; there is no management auth.
Do not expose this port through a public Service, Ingress, port-forward tunnel or
monitor proxy. The web image returns 404 for management paths.

Backend stdout/stderr emits JSON startup, tick-overrun and shutdown events;
no request payloads, display names, invitation codes or private credentials are
logged. Keep ingress/access logging equally conservative: URLs can contain room
codes/names, and neither query strings nor WebSocket bodies should be logged.
Local evidence checks that private reclaim credentials never enter results.

SIGTERM/SIGINT stops ticking/admission, marks health unready, sends existing
players `backend restarting; rooms reset; join a lobby again`, attempts a WS
1012 close and shuts down their sockets, then exits. Sockets are nonblocking;
admission workers have deadlines and process exit does not wait for them.
Chart termination grace is 15s; local real-client/container checks require exit
within 2s under their measured conditions. Unadmitted sockets may simply close.
This is bounded notification, **not match draining or seamless backend recovery**.

## Build and release

```sh
# Native/WASM compilers must be serialized; use the existing toolchain setup.
cargo test --locked
cargo clippy --locked --all-targets
cargo build --locked --release --bin ryggattack-server
mkdir -p dist/server
cp target/release/ryggattack-server dist/server/
scripts/build-web.sh
podman build -f server/Dockerfile -t ryggattack-server:local .
podman build -f web/Dockerfile -t ryggattack-web:local .
ENGINE=podman node scripts/container-smoke.mjs ryggattack-web:local ryggattack-server:local
helm lint charts/ryggattack --strict
python3 scripts/chart-test.py # requires PyYAML
```

Native Linux builds need the existing libudev/Wayland build dependencies and a
C compiler; runtime backend dependencies are glibc/libm/libgcc only. The backend
runtime is Debian trixie (glibc 2.41); compile its executable on compatible glibc
(CI Ubuntu), not a newer incompatible host. Runtime images use nonroot users,
read-only roots, dropped capabilities, no privilege escalation and resource
requests/limits in Helm. Only web needs a bounded `/tmp` emptyDir for nginx.

Forgejo is authoritative. Its `ubuntu-latest` label currently uses a Debian
Bookworm Node job image and the runner's Podman-compatible Docker API. The release
uses `docker.io` with `DOCKER_BUILDKIT=0`, without an unavailable Buildx package;
Dockerfiles set final permissions with portable `RUN chmod` instructions rather
than BuildKit-only `COPY --chmod`. Runtime users/permissions, read-only roots and
all prepublication image/socket checks are unchanged. Local image verification
uses rootless Podman and also exercises the Docker-compatible build path. In CI,
the smoke containers explicitly share the calling job's network namespace,
without host-published ports: a remote Docker daemon's loopback is not the job's
loopback. Local smoke retains randomized loopback-only publication.

1. PR CI runs Rust tests/clippy, JS/UI/startup tests, production/abuse/native TLS
   socket checks, real headless checks, WASM build, and Helm render/semantic tests.
2. Release workflow compiles/tests the backend and WASM serially, builds the
   **separate** images with identical `sha-<short source commit>` tags and OCI
   source/revision labels, and exercises the real hardened images before push.
3. Publication is **main only**, conditional on `PACKAGE_USER`/`PACKAGE_TOKEN`.
   Both immutable image tags must push successfully before desired deployment
   changes; `latest` is only a convenience and never used by the chart.
4. The scoped checkout workflow token needs repository write permission for the
   chart commit. It sets `Chart.yaml` `appVersion` to the shared published tag
   and commits `chore: deploy paired … [skip ci]`. The package token is not used
   as a repository-write credential. No-op updates do not commit. A newer main
   revision causes failure rather than overwriting it. Failed/missing credentials
   or a failed second image never advance the chart.
5. `[skip ci]` prevents a chart version commit from generating another release.
   Flux follows main chart revisions; it does not follow `latest` or image scans.

A manual workflow from a reviewed branch **builds/smokes only**. It deliberately
does not publish or change main. Bootstrap should merge the reviewed source via
the normal main release path, wait for both source-tagged packages and the
post-publication chart commit, and only then create/reconcile the Flux release.
The chart's initial `sha-e0d6439` is an **unpublished backend placeholder**, not
an authorized first deployment. Do not apply it before successful publication.
If chart commit-back is blocked by token policy, retain the old desired state
and repair that permission/explicitly review a version-only commit after proving
both packages exist. Never invent an image tag to get Flux started.

## Helm / infrastructure hand-off

`charts/ryggattack`, release/namespace `ryggattack`, imagePullSecrets default:
`[{name: forgejo-pull-secret}]`. No other values override is required. A single
optional `image.tag` override changes **both** components and must match
`sha-<7..40 lowercase hex>`; empty uses `Chart.appVersion`.

Ingress class **external**, host `ryggattack.modig.online`, TLS Secret
`ryggattack-tls`, cert-manager ClusterIssuer **letsencrypt-cloudflare**:

- `/` Prefix → `ryggattack-web:http` (8080).
- `/ws` Exact → `ryggattack-server:websocket` (8787), no rewriting.
- `ryggattack-server:management` (8788) **has no ingress path**.

The separate infra GitRepository should follow `main`, use the namespace's
read-only git Secret, and include each parent in source ignores:
`/*`, `!/charts/`, `!/charts/ryggattack/`, `!/charts/ryggattack/**`.
Its HelmRelease uses `chart.spec`, `reconcileStrategy: Revision`, this chart and
`forgejo-pull-secret`. Parent owns that manifest, Secrets, merge and live rollout.

Before declaring public success, the operator must verify exact source/tag/digest,
Flux readiness, one backend replica, resource pressure/tick rate, both services,
public trusted certificate and hostname, actual WSS room creation/ready/start,
reconnect, `/ws/` and unrelated upgrade rejection, absence of public management,
and a real browser at the public origin. Local TLS mapping deliberately bypasses
certificate trust only in the fixture and cannot establish those public facts.

## Reset, rollback and migration

Any restart, Recreate update, node loss, uninstall or rollback **destroys all
rooms and private reclaim identities**. Warn players before maintenance.
SIGTERM gives a clear reset message where transport is still working; a crash
cannot. There is no backup/restore or cross-revision session migration. Return to
an ordinary lobby and share a fresh invitation after reset.

For rollback, select a previously verified **paired** web/server source tag,
commit that `appVersion` (or reviewed shared `image.tag`) through Git, and let
Flux reconcile. Do not independently downgrade one protocol peer, change the
replica count, or run `helm upgrade` against a Flux-owned release. The first
phase-5 deployment has no older paired backend tag to roll back to; disabling the
release/Ingress is an infrastructure decision, not restoration of old rooms.

GitHub Pages now publishes a small canonical redirect, not another gameplay
artifact. It preserves room/settings query parameters, replaces obsolete backend
URLs with canonical WSS when joining/creating, and keeps plain/offline entry
plain. Local web artifacts/images still use `web/index.html` and remain playable
without connecting. Old rooms hosted on another backend cannot migrate their
in-memory state simply because a URL redirects. Coordinate the initial Pages
redirect with public bootstrap: its workflow can complete before the first
Kubernetes release/certificate is available. No public readiness is implied.

See `docs/online-deployment-evidence/review-corrected/` for the latest corrected
runtime's local artifacts, source hashes, commands, timings, screenshots and
limitations. The evidence directory's root preserves earlier pre-review results
and must not be substituted for the corrected-source reruns. Public rollout and
trusted public browser/native acceptance are separate from either local fixture.
