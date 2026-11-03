# Reviewed beta production-origin boot repair

Base: `79af998` on `feat/shared-beta`. This package records local TDD and runtime
checks, not publication or live beta recovery. Older failed receipts are untouched.

## Root cause and observed RED

The chart already rendered the beta HTTPS Origin, but `Server::bind_production`
accepted only the canonical production string. The old paired-image smoke also
hardcoded that canonical string, hiding the beta-only startup failure.

`logs/red-server-policy.log` reaches the actual compiled server through a separate
startup subprocess and fails with the reproduced error:

```text
production startup exited 1: Error: Custom { kind: Other, error: "production requires exactly https://ryggattack.modig.online" }
```

`logs/red-beta-workflow.log` independently catches the CI step's missing explicit
beta runtime origin. Both failures are expected RED; they remain immutable.

## Minimal repair and GREEN

The production binder accepts exactly one of the two reviewed HTTPS origins,
never a list or arbitrary host. Production chart defaults and `release.yml` are
unchanged. Missing/empty, HTTP, arbitrary HTTPS, wildcard, whitespace, explicit
ports, trailing paths/slashes and comma lists fail actual startup. The subprocess
regression avoids process-global Rust environment mutation.

The beta CI image smoke explicitly selects the beta origin, passes it through to
the real backend and socket verifier, and preserves it on Recreate. Socket tests
accept the selected origin and native absent-Origin, reject the other reviewed
host, arbitrary HTTP/HTTPS, null/wildcard/empty/malformed/list/duplicate headers,
and retain exact `/ws`, management, abuse, readiness and shutdown checks.
These are WebSocket Origin checks, not a claim of an HTTP CORS middleware.

| Gate | Result | Wall time |
| --- | --- | --- |
| Baseline debug binary build | PASS | 0.981s |
| RED actual beta startup | expected FAIL | 0.348s |
| Corrected debug binary build | PASS | 71.161s |
| Corrected policy/startup + beta socket contract | PASS | 5.663s |
| Default canonical socket contract | PASS | 5.176s |
| Native tests (252 across library/integration targets) | PASS | 196.998s |
| Clippy all targets | PASS, 7 existing warnings + test duplicates | 9.011s |
| Native TLS / server banner / sockets | PASS | 2.188s / 0.096s / 10.373s |
| Real no-display executable | PASS | 2.977s |
| Node tests, explicit TAP reporter (171 tests) | PASS | 6.125s |
| Server / web local container build | PASS | 3.339s / 5.890s |
| Actual paired beta / canonical image smoke | PASS | 9.708s / 9.399s |
| Helm strict lint | PASS, informational icon recommendation | 0.162s |

All chart, beta-chart, beta-pointer (11 cases), beta-workflow, version-update and
prediction evidence Python gates also pass. Exact individual commands, timings,
expected failures, source/binary/image identities and log hashes are in
`receipt.json`. Unchanged warning source files are listed there; warnings were
not introduced or fixed by this patch. No reporter fixture was changed: the
explicit `--test-reporter=tap` run passes even the prior reporter-sensitive test.
The older failed Node receipt remains historical, not rewritten as success.
Raw command output is preserved byte-for-byte. `native-tests.log` ends with Cargo's
extra blank line; that evidence-only `git diff --check` finding is intentionally
retained. Source/document whitespace checks pass with raw logs excluded.

## Scope and container limits

Only changed Rust module: `src/server.rs`. `src/lib.rs:22-23` excludes it under
`cfg(not(target_arch = "wasm32"))`; all client code/assets are unchanged relative
to the recorded base. No WASM build is required or claimed for this server-only
repair.

The local paired check uses the actual production Dockerfiles, nonroot/read-only
roots, dropped caps and real sockets, Recreate and SIGTERM. The server executable
is this patch's native debug build stripped of debug sections, **not** the CI
release-profile executable. The web artifact is a read-only copy of the existing
`ryggattack-mobile-touch/dist/web` bytes with hashes in the receipt: this is
explicitly a **mixed-source packaging/socket check**, not fresh same-SHA browser
compiler provenance or rendered gameplay acceptance. No mobile source was edited.
Same-head web artifact binding and release build remain the real CI gates.

Local rootless Podman uses random loopback ports; the Forgejo calling-job network
namespace was not recreated here. No push, remote PR update, external deployment,
cluster change, production restart or live recovery is claimed. The parent must
review this local commit, update PR23/mobile24, follow CI publication and separately
verify Flux/live beta readiness and actual public HTTPS/WSS gameplay.
