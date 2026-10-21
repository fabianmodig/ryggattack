# Portable release-builder correction

The first post-PR11 release (run 45 / API315, source `ebdb8f4`) failed during
package installation: the actual `ubuntu-latest` runner job uses
`node:22-bookworm`, whose configured repositories do not supply `docker-buildx`.
No backend/WASM build, image smoke, registry publication or chart commit-back
was reached. Both new commit image tags were authenticated 404s; the bootstrap
chart was not deployed.

## Correction

- Use Debian's existing `docker.io` client with `DOCKER_BUILDKIT=0` against the
  runner's Podman-compatible Docker API; no Buildx package or plugin is needed.
- Replace BuildKit-only `COPY --chmod` with portable `COPY` then `RUN chmod`.
- Preserve exact final modes: nginx configuration `0644`, backend executable
  `0555`. Keep runtime users 101/10001, rootfs read-only, capabilities dropped,
  same origins/routes, safe logs, probes, paired publication and chart sequencing.
- Add `portable-container-build.test.mjs` to the chart CI job, and update the
  semantic contract to prevent this unavailable-package dependency returning.

## Actual verification

The new regression was run before the correction and failed on the missing
portable builder contract. It passes after the correction, as do real Helm
render/semantic checks and strict Helm lint.

Both **real** images were then built using Docker CLI **20.10.24**, API1.41,
against the local rootless **Podman5.4.2 Docker-compatible API**, with BuildKit
explicitly disabled. This verifies the intended client/API builder path, not
merely a direct `podman build` approximation. The exact previously tested native
release and WASM bytes were reused; no Rust/Cargo/assets/client-module inputs
changed. Both builds succeeded, including actual permission changes and nginx
configuration validation.

The same Docker client/API then ran the full paired-container smoke: **PASS in
9.022s**, retained in `portable-classic-containers.log`. It exercised exact
HTML/JS/WASM/module bytes and MIME, nonroot/read-only/cap-drop execution, private
query/referrer log checks (including missing/malformed requests and both streams),
production socket/rate policy, same-image Recreate and real client SIGTERM/reset
with exit0. Local image IDs:

- web: `ac7d6be42dcca0f6b5954249327a1374e550eae5048c73782909c9bd6dd3b61a`
- backend: `eabb4443c2b87d1760734ec46d34cdc5dcd34fbc0f7c66f9ded74d2d0cc49476`

These are **local build proofs**, not registry manifest digests or publication
claims. The release must still pass on the actual CI runner and publish both
new main-revision images before the chart is bootstrapped in the cluster.
