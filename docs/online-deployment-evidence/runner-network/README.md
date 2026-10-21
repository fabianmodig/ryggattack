# CI smoke network-namespace correction

Release48/API318 for `ec9ab8e` successfully built the backend and optimized WASM,
then both runtime images. It failed at `web readiness` before publication or
chart commit-back. The deployment bootstrap remained blocked.

## Proven root cause, on the actual runner

The Node job container is a Docker API **client**, not the Podman daemon host.
A test container publishing `127.0.0.1:<random>` exposes that port on the daemon
host's loopback, not the Node job's loopback. On runner LXC108, using the actual
CI-built web image, the same hardened container produced:

- daemon-host HTTP request: **200**;
- separate `node:22-bookworm` job request to that published loopback port:
  **ECONNREFUSED**;
- a hardened web container sharing the job's network namespace: job HTTP request
  to `127.0.0.1:8080`: **200**.

The application was healthy. Increasing a readiness timeout or changing nginx
would not correct the namespace mismatch.

## Fix and safeguards

The release explicitly supplies `CONTAINER_SMOKE_NETWORK_CONTAINER="$HOSTNAME"`.
The smoke joins only that validated container-ID network namespace, with **no
host-published ports**, using job-local 8080,8787,8788. Default local smoke mode
still publishes only randomized 127.0.0.1 ports. Unit tests reject malformed
namespace IDs, invalid ports and non-loopback published addresses.

Runtime users, read-only roots, capability drops, no-new-privileges, Origin/path
policy, private-log checks, Recreate and client SIGTERM assertions are unchanged.
Readiness errors retain the final cause; owned-container State and safe application
logs are captured before cleanup, without dumping container environment values.

## Actual corrected full smoke

The corrected script was executed **inside a fresh node:22-bookworm job container
on the actual runner**, using its Debian Docker20.10 client and actual runner
Podman Docker-compatible socket. Both actual CI-built `sha-ec9ab8e` images passed
the complete hardened-container/privacy/socket/rate/Recreate/SIGTERM check in
**8.955 seconds**, exit0. The raw result is `runner-network-proof.log`.

For this focused network reproduction, expected static asset bytes were extracted
from that actual CI image after the original job had been removed. Therefore its
hash comparison is a serving consistency check, **not an independent
compiler-to-image provenance proof**. The real release still compares served
bytes against its freshly built `dist/web`, and must pass before publication.

Three helper tests were observed RED before implementation and GREEN afterward.
The regression is wired into CI alongside the portable builder, chart and existing
runtime gates. No Rust, Cargo, asset, browser runtime, Dockerfile, chart, registry
or Kubernetes deployment changes were made. Ephemeral diagnostic job/test
containers were removed after the checks.
