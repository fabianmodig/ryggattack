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

- `git.modig.online/fabianmodig/ryggattack/beta/web:sha-<artifact-id>`
- `git.modig.online/fabianmodig/ryggattack/beta/server:sha-<artifact-id>`

The shared 40-hex artifact ID is derived from a fresh random UUID per execution,
not from Forgejo's run/attempt counters. Same-SHA reruns get a new pair and
never overwrite an existing pair or production `sha-*`/`latest`. The existing
chart tag regex is unchanged. These beta tags identify **artifacts**, not source
commits; OCI revision labels and pod annotations identify the actual PR SHA.
Production image names/tags and `release.yml` are unchanged.

After **both** pushes succeed, `scripts/beta-pointer.py` copies the PR's chart
into a commit on the stable **beta-deploy** branch, setting the paired image
names/tag, beta host/Origin and source SHA/PR/run metadata. It never pushes main.
The entire chart is delivered, so PR chart changes are exercised too, without a
semver bump. Only this branch's chart is a delivery source; its other files are
not a source checkout for builds. Do not merge the generated branch into main.

## Concurrency and failures

Policy: **latest successful eligible pointer completion wins**, across all PRs.
Existing per-ref CI cancellation avoids obsolete work. There is deliberately no
global workflow concurrency group that would cancel or coalesce other PRs.
Instead the one global `beta-deploy` ref is the serialization point: all updates
are ordinary non-force pushes, rejecting competing stale parents. The loser
fetches the new tip and retries up to eight times. No force-push, rebase of a
candidate source tree, or production commit-back is used. A real temporary-Git
regression creates a concurrent winning commit and proves the retry is its
child, not a history overwrite.

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
Therefore the new job requires **BETA_DEPLOY_TOKEN**, a repository-write token
for this app repository (also used for the PR API). `PACKAGE_USER` and
`PACKAGE_TOKEN` publish packages as in the existing release pipeline. No cluster
credential, kubeconfig or Kubernetes network access is required in CI.
Provision secrets through the operator's existing secure process; never commit
them or send them in chat. Protect main from the beta bot identity and permit
that identity to create/update only `beta-deploy` via branch protection. Verify
these effective rights before supplying its token. Do not rely on a GitHub
`permissions:` block; it does not grant Forgejo PR write access. Package rights
should be minimized to beta publication where the registry/account model
allows it. Trusted same-repo workflows can access the configured secrets;
branch protection and contributor trust are prerequisites, not optional claims
that the workflow can enforce on itself.

Artifact upload/download use Forgejo-hosted v3 actions and same-run artifact
names, not GitHub's v4 service. The first real run must verify artifact transport
and PR token checkout/push on the installed runner. UUID allocation intentionally
does not depend on unverified `run_attempt` rerun semantics.

The companion nixos-config change defines isolated Flux resources. It is
suspended and names a namespace-scoped impersonation ServiceAccount with **no
new RBAC grants in this change**. Bootstrap, RBAC approval and live acceptance
are documented there. Do not bypass this by reconciling PR charts as
helm-controller's cluster-admin identity.

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
