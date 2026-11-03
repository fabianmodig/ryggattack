# Shared beta delivery

URL: **https://ryggattack-beta.modig.online**, secure same-origin `/ws`.
This is one shared environment, not one preview per PR. A replacement restarts
both single-replica workloads (`Recreate`); in-memory rooms and ongoing games
are lost. Production remains `main` → production chart/images/host.

## Eligibility, gates and delivery

Ordinary `pull_request` events (opened, synchronized, reopened) run the existing
native tests, clippy, runtime/abuse/socket/headless checks, chart contracts and
web build. The new `beta` job needs **test + chart + web** to succeed. It reuses
the web job's run-scoped artifact, verifies its source-SHA receipt, compiles the
release backend, builds the two runtime images and runs the existing hardened
paired-container smoke before publishing either image.

Only **trusted same-repository PRs** publish. Fork PRs run tests/web builds only,
do not upload the beta artifact or enter the secret-bearing job, and cannot
advance beta. There is no `pull_request_target` checkout or privileged fork
approval path. Same-repository contributors can change workflows, scripts,
images and Helm templates: they must be trusted with publisher secrets and
with beta workload control. A repository equality condition is not a sandbox
against malicious collaborators.

Beta images are separate package repositories:

- `git.modig.online/ryggattack-beta/ryggattack/web:sha-<artifact-id>`
- `git.modig.online/ryggattack-beta/ryggattack/server:sha-<artifact-id>`

The shared 40-hex artifact ID is derived from a fresh random UUID per execution,
not from Forgejo's run/attempt counters. Same-SHA reruns get a new pair and
never overwrite an existing pair or production `sha-*`/`latest`. The existing
chart tag regex is unchanged. These beta tags identify **artifacts**, not source
commits; OCI revision labels and pod annotations identify the actual PR SHA.
Production image names/tags and `release.yml` are unchanged.

After **both** pushes succeed, `scripts/beta-pointer.py` copies the PR's chart
into a commit on the stable **beta-deploy** branch of the private isolated repo
`https://git.modig.online/fabianmodig/ryggattack-beta-deploy.git`, setting the paired image
names/tag, beta host/Origin and source SHA/PR/run metadata. It never pushes main.
The entire chart is delivered, so PR chart changes are exercised too, without a
semver bump. The first commit is an orphan in a fresh temporary Git repository:
its tree contains **only `charts/ryggattack`**, with no app source or source
history. Later commits descend only from the destination branch. Source `main`
and all source refs remain untouched. Do not merge generated delivery commits
into the source repository. No production repository/package migration occurs.

## Concurrency and failures

Policy: **latest successful eligible pointer completion wins**, across all PRs.
Existing per-ref CI cancellation avoids obsolete work. There is deliberately no
global workflow concurrency group that would cancel or coalesce other PRs.
Instead the one global `beta-deploy` ref is the serialization point: all updates
are ordinary non-force pushes, rejecting competing stale parents. The loser
fetches the new tip and retries up to eight times. No force-push, rebase of a
candidate source tree, or production commit-back is used. A real temporary-Git
regression creates a concurrent winning commit and proves the retry is its
child, not a history overwrite. A second regression races the first orphan
branch creation and proves the losing publisher retries on that first winner.
After every successful push, an authenticated `ls-remote` of the exact
`refs/heads/beta-deploy` must equal the local commit before reporting success.
A concurrent advancement during read-back fails closed rather than claiming
this candidate is still current; the already-pushed commit is not rolled back.

The publisher reads the live PR API after fetching each parent and again just
before pushing. Closed PRs, forks and changed heads are skipped; conflict retries
repeat these checks. Thus an older same-PR run cannot replace a newer published
head. API reads and Git pushes are not a cross-service transaction: a head can
change in the tiny interval after the final check. An old push in that interval
can briefly precede the new head's successful delivery, but cannot overwrite a
newer delivery commit without a rejected push/recheck. Reruns of the current
same SHA are intentionally eligible and may replace another PR's beta.

Failed gates, image builds, smokes, pushes or API queries retain the previous
desired state. A partially published pair is unused; clean orphan beta packages
only after checking current desired state/rollout history. Exhausted Git retries
fail the job without force. A successful pointer is **not** proof of a healthy
live rollout: Flux convergence, image pull, TLS and gameplay remain acceptance
gates. A bad runtime/chart may still fail to deploy after publication.

## Forgejo-specific setup (operator, after review)

Read-only version API on 2026-11-03 reported `17.0.1+gitea-1.22.0`.
Matching references (not GitHub assumptions):

- [Forgejo v17 security](https://forgejo.org/docs/v17.0/user/actions/security/)
- [Forgejo v17 PR trust](https://forgejo.org/docs/v17.0/user/actions/security-pull-request/)
- [Forgejo v17 workflow reference](https://forgejo.org/docs/v17.0/user/actions/reference/)
- [Flux HelmRelease, revision reconciliation and impersonation](https://fluxcd.io/flux/components/helm/helmreleases/)

Forgejo PR automatic tokens are read-only, including same-repository PRs.
Credentials are deliberately separated:

| Credential | Use and effective boundary |
| --- | --- |
| `${{ github.token }}` | Ordinary source checkout token, also exported as `PR_READ_TOKEN` for source live-PR API reads. No source write permission needed. |
| `BETA_DEPLOY_TOKEN` | Bot `ryggattack-beta`, repository-scoped write to **only** private `fabianmodig/ryggattack-beta-deploy`; no source repository rights. |
| `BETA_PACKAGE_USER` / `BETA_PACKAGE_TOKEN` | Bot-owned `ryggattack-beta` package publication; separate owner from all production packages. |

The beta job never uses production `PACKAGE_USER`/`PACKAGE_TOKEN`, and
`release.yml` stays unchanged. The delivery bot must **not** be granted source
access. This avoids relying on branch protection to separate a shared source
write token from the automatic production release identity. Same-repository
contributors are still trusted with beta secrets and workload control, not
production source or production package publishing credentials via this job.

The isolated Git remote is named `beta`. Its token is Basic-auth encoded only
in memory, supplied through subprocess `GIT_CONFIG_*` environment entries for
`http.extraHeader`, never a remote URL, argv or persistent Git config. Source
checkout auth is not copied. Git/API private diagnostics are suppressed on
failure so a remote cannot echo credentials into logs. The source read token is
used only by the PR API request; the destination write token is never sent to
that API. A local absolute-path destination override exists for temporary-Git
regressions only; production defaults to the exact isolated HTTPS URL above.
No cluster credential, kubeconfig or Kubernetes network access is required in CI.
Do not rely on GitHub `permissions:` to grant Forgejo PR write access.

Operator provisioning metadata on 2026-11-03 reports bot ID 3, destination repo
ID 27, default branch name `beta-deploy`, **no seeded branches**, and all three
beta Actions secrets installed. This is a provisioning receipt, not publishing
or rollout acceptance. Actual bot-owned image push, first chart-only branch
creation, automatic source-token API access and runner artifact transport must
still be exercised by reviewed CI. Never record credential values in evidence.

Artifact upload/download use Forgejo-hosted v3 actions and same-run artifact
names, not GitHub's v4 service. The first real run must verify artifact transport
and automatic-token source checkout/API plus destination-only push on the
installed runner. UUID allocation intentionally
does not depend on unverified `run_attempt` rerun semantics.

The companion nixos-config work owns isolated Flux resources, the private
destination Git URL/read-only credential and namespace-scoped impersonation
RBAC. Activation and live acceptance are documented there; no infra or mobile
files are changed by this app revision. Do not bypass namespace isolation by
reconciling PR charts as helm-controller's cluster-admin identity.

## Local tests and TDD

Run from this repository with Python/PyYAML, Node and Helm (no Rust build):

```sh
python3 scripts/chart-test.py
python3 scripts/beta-chart.test.py
python3 scripts/beta-pointer.test.py -v
python3 scripts/beta-workflow.test.py
helm lint charts/ryggattack --strict
```

Observed RED → GREEN tracer slices:

1. Beta rendering initially failed `KeyError: 'annotations'` (no source metadata;
   ingress/Origin also hardcoded). After chart parameterization, beta and the
   unchanged production render contracts passed.
2. Real temporary-Git publication initially failed `missing beta pointer
   publisher`. After implementation it passed; expanded coverage also proves
   stale/closed/fork rejection, the final head recheck, non-force conflict retry
   and malformed API failure retaining desired state (six cases).
3. Workflow contract initially failed `missing gated beta publication job`.
   After wiring the gated job/artifact reuse it passed.

No Cargo/WASM/container build was run locally for this configuration change
while the separate mobile work used shared build caches. Required build/runtime
CI gates remain in place. Publishing, actual rerun/concurrency behavior, Flux
RBAC and live HTTPS/WSS acceptance are not implied by these local tests.

### Isolated-delivery revision (2026-11-03)

Prior prototype/failure evidence remains unchanged; the earlier six-case result
above describes the original source-branch prototype, not isolated acceptance.
The revised RED slice failed because the destination had no `beta-deploy` ref
(the old publisher wrote to source origin). GREEN creates an orphan chart-only
initial tree and separate linear destination history. A second RED slice
rejected checkout with the destination token; GREEN uses `${{ github.token }}`
and beta-only package credentials while preserving all full CI gates and web
source-SHA/OCI provenance checks.

The expanded temporary source + destination bare-Git/API suite covers source
ref/history isolation, chart-only first tree, source-token API authorization,
stale/closed/fork rejection, final live-head and final API-failure checks,
initial creation races, existing-parent non-force retry, subprocess-only
private destination auth, and exact-ref read-back (including a transport that
falsely reports push success). No external writes or Cargo/WASM builds are
part of these local regressions. Independent review and actual CI publication
remain separate gates after this local commit.

Local verification of this isolated revision: Helm strict lint and all seven
Python gates passed (including 11 pointer cases). The 168-test Node nonbuild
run passed 167 and failed the preexisting solo privacy fixture's reporter-text
assertion: Node 22.23.4 emitted `ℹ tests 1` while the unchanged test expects
`# tests 1`. Its nested privacy test itself passed. No Node/runtime/mobile file
was modified to work around this. A proposed reporter-pinned rerun was blocked
by the execution approval layer and was not executed or retried. The original
failure log is retained separately; this is **not** an all-green gate claim.
