# Authoritative online arena presentation

Implementation scope: `t_c075bec8`, continuing the uncommitted `feat/visual-fidelity` candidate at base `dfbd361580ab6861097141ff982766dbae9da6d2`. The preceding missile/prop slice and its historical evidence remain intact. This document supplements `visual-renderer.md`; its statements that the full environment is deferred describe the preceding slice, not this candidate.

## Authority and shared construction

The arena choice comes only from an identity-matching lobby: room, participant and seat must agree with the network adapter. Lobby and Countdown can display the chosen environment, but only Running consumes body snapshots. Forest → Volcano → Forest changes the environment independently of simulation ticks. Missing/mismatched lobby identity and Leave remove the cosmetic hierarchy.

The existing offline builders are now separated into reusable visual-only functions:

- `scene::floor_and_walls`: the same inner grass floor, slightly lowered outer ground and four boundary walls. Offline Ground markers and shadow exclusions are attached by the offline setup; no gameplay is constructed by this function.
- `arena::volcano_parts`: the actual basalt craters, emissive lava river, crater rims and flush pools, without `rebuild_forest` or scenery creation. Offline `apply_arena` retains its existing gameplay regeneration and invokes this pure builder for decorations.
- `scenery::backdrop_meshes`: the source material-batched, patch-culled unreachable woods, preserving the sparse/full density rules and effective capped volcanic rocks. Offline `spawn_backdrop` attaches its existing Backdrop markers to these meshes.

Online `environment::present` uses these functions without installing Forest, RailMap, Scenery, Transient, detonation or gameplay systems. All reachable props still come exclusively from server Body descriptors and disappear only when authoritative snapshots remove them. Cosmetic backdrop props cannot be destroyed and are selected only from `!can_be_wrecked()`. There is no client-created collider, damage target, topology change or simulation substitute.

The deterministic seed-zero layout used for cosmetic woods and harmless lava placement is not the server's scenery seed and is not claimed to reproduce its reachable props. The received edge list is used read-only to avoid placing pools on rails. Source decorations keep their existing clearance: river/crater footprints outside the walls; small pools over 1.2 units from rails and below 0.03 world height. The camera uses the source framing, including the right-flank crater. No UI/input element is added or intercepted.

## Cache and lifetime

One meshless Environment root owns every floor, wall, lava, crater and background mesh as children. Theme/room changes despawn the entire old hierarchy. Both arena presentations are prepared once per authoritative topology and keep strong mesh/material handles; repeated frames, same-topology room changes, rematches and reconnects do not allocate another material set. Forest and Volcano use separate cached background palettes, so a hidden Forest cache does not leave any Forest entity in the Volcano world.

Sparse source backdrop is the online baseline, including low-quality play: it keeps mixed Forest silhouettes/materials and volcanic rock patches while avoiding hundreds of independent tree draws. Dynamic lighting, particles, trails, debris and scorch remain outside this slice.

Static scenery survives same-theme round/reconnect barriers, while the existing visual epoch replaces match-local replicas even if Lobby/Countdown are coalesced away. Native Leave/Create/Join keeps the existing railway and cart models hidden until the next authoritative frame instead of reallocating models/materials on every rejoin. An actually different received topology invalidates railway/cart/environment roots once and rebuilds presentation from the new edges. The cache holds only the current topology's two themes, not an unbounded room history. A fresh process/reload reconstructs deterministic geometry without retaining old ECS IDs.

## Verification boundary

`src/online/environment/tests.rs` exercises the actual renderer and environment systems with CPU-side Bevy assets and actual authoritative source frames. It covers repeated Forest/Volcano/Forest selection; Lobby/Countdown/Running; coalesced same-theme round barriers; another room; reconnect; Leave/rejoin; fresh app/reload vertex equality; invalid identity; changed authoritative topology; and actual native Leave/Create controls. Assertions check one environment root, recursive cleanup, replica ID replacement, stable cached asset counts, effective palettes, lava/crater presence and clearance, no local gameplay scenery/resources, and hidden cart reuse during rejoin.

The full native suite also retains source/replica prop geometry/material parity, missile hierarchy/owner/pose tests, explosion world-space bounds, pickup characterization, offline/server parity, rear contact and round reset coverage.

Exact source archives, successful and failed execution receipts, native hashes and durations are under `verification/arena-presentation/`. Native CPU/ECS verification is not graphical acceptance. No WASM rebuild, rendered browser acceptance, native graphical session, public room, publication, merge, deployment or production restart is claimed here. The dependency-gated final web acceptance card performs the single final-source WASM freeze and rendered two-client/low-quality/offline checks after independent review. The accepted anonymous-bearer theft limitation is unchanged.
