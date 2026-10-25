# Authoritative missile and destructible-prop presentation

Local implementation for `t_823da1c4`, continuing the reviewed protocol-v4 descriptor work on uncommitted `feat/visual-fidelity`, base `dfbd361580ab6861097141ff982766dbae9da6d2`. This is not a publication, deployment or browser-fidelity acceptance.

## Model construction

- Projectiles use the existing source missile hierarchy: owner-painted cylinder, pale cone nose, two crossed fins, emissive owner-colored exhaust. The source spawn path and online renderer share `combat::missile_visual`, which only attaches visual children to an existing entity. Only the source firing path installs `Projectile` and its velocity/lifetime/trail state.
- Projectile roots use the authoritative position, quaternion and scale. Their forward axis is local -Z; the nose is at z=-0.35 and exhaust at z=+0.28. Owner is the authoritative cart seat, including bots and eliminated owners, not a local participant identity.
- Props use the existing `scenery::parts` model rules and `ForestAssets`. Asset construction is separated from `Forest::new`, so an online view does not generate a forest layout or infer kind/variant from a collider or RNG. Pine, Birch, Bush, Rock, Stump and Fern keep the source meshes, per-part placements and materials. Variants use the source rules (some kinds intentionally have identical variants).
- A prop root stands at `(body.position.x, 0, body.position.z)`, with the body quaternion and uniform `body.prop.scale`. The wire position.y and scale remain the authoritative collider envelope, not model dimensions. Those collider fields, the source collider and all gameplay rules are unchanged.
- Volcano's live effective Rock identity and capped scale come from the descriptor, not a client-side conversion. Rock material uses the existing lobby arena setting and the shared source palette. The same cached material is updated only when the arena changes, not allocated every frame. Full floor, backdrop, lava, crater and boundary presentation belongs to the arena card.

## Lifetime and authority

Replica identity is match-local body ID. Repeated snapshots update the same root transform and retain its children. Complete-snapshot removal and absent frames recursively remove the root and its visual descendants. Room-switch cleanup includes meshless replica roots, not only mesh-bearing roots and carts.

The network adapter exposes a presentation-only epoch when its existing Welcome/non-running mailbox barrier is consumed. This survives a coalesced Lobby/Countdown/Running/fresh-State sequence where the renderer never observes an absent frame. The view invalidates replicas on epoch or room/participant/seat changes, and only copies frames for an identity-matching Running lobby. A coalesced reconnect/rematch cannot attach an old hierarchy to a reused numeric ID. Normal rounds keep the source's monotonic body allocation/removal semantics.

The online schedule remains controls, network input and snapshot rendering only. Visual roots have no `Projectile`, `Scenery`, `Transient`, local detonation, damage, collision or scoring components/systems. Source gameplay and offline rendering retain their existing systems. Pickup meshes/scales and the canonical explosion mesh/geometry are unchanged; the old generic-projectile characterization is intentionally replaced by source-missile hierarchy assertions while keeping pickup characterization.

## CPU/ECS verification

`src/online/visual_tests.rs` drives the actual source forest spawner and missile firing system, real frame serialization and actual online renderer using CPU-side Bevy assets. It compares child counts, mesh vertex positions/indices, local part transforms, transformed per-part world bounds and source materials, not merely root transforms.

The prop matrix covers every original kind, both variants, scales 0.75/1.15/1.5/2.2, yaws 0/0.7/-2.4, Forest and effective/capped Volcano forms. Missile coverage includes all four owners, yaw/pitch and nonuniform scale, forward nose/rear exhaust, owner paint/glow, stable roots/children and subsequent authoritative pose updates. Lifecycle coverage includes removal, Lobby, Countdown, room/participant/seat mismatch, Leave, reconnect/coalesced-round invalidation, same-ID replacement with different art and native room-switch hierarchy cleanup. The real loopback coalescing regression covers both no replacement frame and a valid fresh State in the same pump.

Existing explosion world-vertex bounds, pickup geometry, shared simulation/parity, rear-contact, scoring/reset and descriptor suites remain in the full native gate. No graphical native session, WASM build, rendered browser replay or public-room acceptance is claimed. Those web artifact/fidelity gates remain after the arena slice. The accepted anonymous-bearer credential-theft limitation is unchanged.

Evidence and exact source/artifact hashes: `verification/visual-renderer/`. Parent descriptor proof remains separately under `verification/visual-descriptors/`; it is historical descriptor evidence, not proof of these renderer changes.
