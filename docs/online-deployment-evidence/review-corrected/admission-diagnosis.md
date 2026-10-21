# Canonical local admission diagnosis (4136c80)

Scope: only `scripts/production-browser.mjs`; no runtime, deployment, or remote changes. Existing phase5-review-production* results retained.

## Reproduction

Full-proc same-user transient unit command (working directory `/home/hermes/src/ryggattack-deployment`):

```
systemd-run --user --pipe --wait --collect --quiet --unit=ryggattack-admission-diag-2 --working-directory=/home/hermes/src/ryggattack-deployment /usr/bin/bash -lc 'export PLAYWRIGHT_NODE_MODULES=/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json; time node scripts/production-browser.mjs verification/phase5-admission-proxy-diagnosis'
```

RED exit 1, real 153.124s, `ProductionB actual form admission`. Earlier command using `/usr/bin/time` exited 127 before running because that binary is absent; shell `time` works.

Boundary evidence: A upgrade 2942.785ms, 101 at 2950.883ms; B upgrade 77710.788ms, 101 at 77716.336ms, upstream close 83385.845ms. B renderer trace: construct 74133.4ms, error 79806ms, close 79808ms; no open, Hello, or Welcome. A remains connected in Lobby. No page/console errors; credential-negative scan true. Artifact hashes are unchanged and recorded in results.json. Therefore Origin/path policy and HTTP rejection are excluded: both native upgrades succeed promptly; B cannot process its browser open event before transport closure.

## Ranked hypotheses and predictions

1. Shared Chromium GPU-process/SwiftShader cold-start work blocks browser B's lifecycle; separate Chromium processes (still exactly two actual rendered clients) should admit both without changing server deadlines.
2. CPU-wide software-render contention rather than shared GPU process: isolation will still fail; stage cold renderer initialization before simultaneous activity if measurable.
3. Proxy upgrade/read-ahead problem: isolated process will fail similarly; compare response and renderer event times. Immediate identical 101s and working A make policy failure unlikely.
4. Runtime regression: only if independently reproducible with renderer-free/old-source differential; do not change production without a failing targeted regression.

## Experiment result / corrected final replay

Isolating browser processes passed twice, with no production changes, timeout changes, or removed assertions:

- `ryggattack-admission-isolated-1`, output `verification/phase5-admission-isolated-browser`: exit 0, real **126.420s**, 1163 identical shared authoritative ticks, zero unexpected errors, credential-negative scan true.
- `node --check scripts/production-browser.mjs && git diff --check && systemd-run --user --pipe --wait --collect --quiet --unit=ryggattack-admission-isolated-2 --working-directory=/home/hermes/src/ryggattack-deployment /usr/bin/bash -lc 'export PLAYWRIGHT_NODE_MODULES=/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json; time node scripts/production-browser.mjs verification/phase5-admission-isolated-browser-final'`: exit 0, real **123.560s**, **1135** identical shared ticks, zero unexpected errors, credential-negative scan true.

Final B trace (navigation-relative): construct 2430ms, open 2450.8ms, Hello 2452.7ms, Welcome 2457.1ms. Previously B had no open/Hello and failed after its ~74-second cold startup. Final A: construct 2125.4ms/open 2147.5ms/Hello 2150.2ms/Welcome 2164.4ms; secure reconnect construct 49654.2ms/open 49664.5ms/Reconnect 51409.6ms/Welcome 51414ms. All three upstream responses were HTTP 101. Both renderers remain active together throughout gameplay, keyboard intent, outage/bot takeover, private-identity reconnect, and held-input neutralization. This canonical harness retains its original single-client outage assertions; the separate simultaneous-reconnect harness was not modified or rerun here.

**Root cause:** fixture artificial contention from two contexts sharing one Chromium/SwiftShader GPU process at cold startup. Changing only browser-process isolation removes both the ~74-second startup blockage and absent open/Hello while the same server/artifact passes every existing assertion. This is evidence for fixture isolation, not a production timeout extension or runtime regression. No WASM rebuild was needed.

Final script SHA256 `1dd057622271630b6f997dc63d4ae16c360be3119715921bb4526fad3809919d`; native `src/server.rs` SHA256 `13d4a34447103497e2a95a28340bade02f8ad6212b1682e43102a9d8811e29e2`; executed debug backend SHA256 `6cd89289b4cb98310d8beaad2eb1aa55099096986050747ec5c11fc486c033a4`. Parent advanced HEAD to `60ca9a3` for its independent live-harness update during diagnosis; backend source/artifacts were unchanged. No builds were run.

Diagnostics keep only HTTP status lines and protocol message kind/timing, never WS payloads. Success/failure evidence runs retain the credential-negative scan. Output directories contain ignored generated local TLS keys: do not force-add entire directories. Earlier failing outputs are preserved. Screenshots show the real rendered online forest scene, both players Running, no visible error. Local self-signed canonical-origin proof only; no public TLS/cluster deployment claim.
