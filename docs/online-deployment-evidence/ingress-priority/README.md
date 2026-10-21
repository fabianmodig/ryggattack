# Isolated Traefik WebSocket priority regression

Scope: chart-only routing correction; **not canonical public acceptance**.
Runtime sources, images and chart `appVersion: sha-205d048` are unchanged.

## Actual controller reproduction (provided by the parent deployment probe)

`controller-proof.json` records a temporary distinct diagnostic HTTP Host on the
actual external controller, using canonical trusted TLS SNI and the same running
web/server backends:

- Combined Ingress, without an explicit priority: `/ws` returned **404** with
  `Server: nginx/1.30.5` (the web backend, not the WebSocket service).
- Added separate Exact `/ws` Ingress with priority **100**: returned **101**.
  The bounded curl observation subsequently exited 28 because the upgraded
  connection remained open; 101 establishes routing/upgrade, not gameplay.
- `/ws/metrics` still returned **404** from nginx, preserving exact-path scope.
- Canonical production Ingress specification and certificates were unchanged.
  Both temporary diagnostic Ingresses were removed; the parent also verified
  their absence with API readback.

This experiment did not route the canonical HTTP Host through the proposed
chart. Trusted canonical browser room creation/start/reconnect acceptance
remains blocked until reviewed main-chart rollout; neither this evidence nor a
server dry run claims a deployment.

## Strict chart RED/GREEN

`chart-red.txt` was captured after updating only `scripts/chart-test.py`, before
changing the original single-Ingress template. The real Helm-rendered chart
failed with exit 1:

```text
AssertionError: split web and WebSocket Ingresses for independent Traefik priorities
```

The minimal template change then passed the same command (exit 0), recorded in
`chart-green.txt`. Existing security, probes, private management, immutable paired
publication, image override validation and Pages migration assertions remain.
The regression requires exactly two Ingresses with priorities 100 > 10, exact
backends/path types, identical external class/canonical host/TLS Secret and the
issuer annotation only on the existing web Ingress.

Commands (run from the repository root; Python requires PyYAML):

```sh
python3 scripts/chart-test.py
helm lint charts/ryggattack --strict
helm template ryggattack charts/ryggattack --namespace ryggattack \
  | kubectl apply --dry-run=server -f -
node --test scripts/container-smoke-network.test.mjs
node scripts/portable-container-build.test.mjs
```

The local test used the existing PyYAML interpreter at
`/home/hermes/.hermes/cache/scratch/ryggattack-phase5-tools/bin/python`.
Helm strict lint passed (only the optional icon recommendation). The server dry
run accepted the NetworkPolicy, two Services, two Deployments and two Ingresses;
it made no persistent cluster writes. Full Rust/WASM CI remains an exact-PR-head
remote gate; no heavy local compilation is needed for unchanged runtime sources.
