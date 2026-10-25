# Authoritative visual descriptors (local slice)

Worktree: `/home/hermes/src/ryggattack-visual-fidelity`, branch `feat/visual-fidelity`, based on `dfbd361580ab6861097141ff982766dbae9da6d2`.
This is an uncommitted descriptor implementation, not a release or rendered-fidelity acceptance. Continue this same worktree for the later renderer and arena slices only when explicitly selected. Do not merge this slice separately.

## Protocol and compatibility

The network protocol advances from 3 to 4. This is deliberately incompatible: the v4 client requires authoritative projectile and scenery identities and must not guess missing art from collider geometry, names or RNG. `Hello` and `Reconnect` reject any other version; the client also checks the welcome version. There is no v3 fallback or migration of an active room. A future authorized release must pair matching server/web artifacts and require old clients to reload. No rollout is authorized here.

Current executable smoke scripts use v4; the socket abuse script explicitly rejects v3. Historical source-at-pass/evidence copies remain unchanged at their original versions. The HUD startup unit fixture is a historical identity value, not a network handshake. Do not run the v4 production probe against the still-v3 public service.

## Body descriptor contract

Existing body kind, position, rotation, scale, stable monotonic ID and complete-snapshot removal semantics are unchanged.

- Projectile (`kind = 0`): `owner` is the actual live `Projectile.owner`, a cart seat 0..3, not a participant ID, human name, connection identity or authentication assertion. `prop` is null. Rotation and scale remain the live transform.
- Scenery (`kind = 3`): `owner` is null; `prop` contains `kind`, `variant`, `scale`. Kind is one of `Pine`, `Birch`, `Bush`, `Rock`, `Stump`, `Fern`; variant is 0 or 1. Scale is finite and within the source generation envelope 0.75..2.2.
- Speed, all-hits and explosion (`kind = 1, 2, 4`): both descriptors are null.

`Prop` is retained as a component on the same live scenery root already used for collisions, after `Forest::themed_prop` chooses its effective appearance. Volcano therefore reports Rock and the actual scale capped at 1.5, preserving the source variant. The terrain/theme authority remains the existing room settings arena; no redundant arena field is added to a frame.

For scenery, `Body.position.y` is still half the collider height and `Body.scale` still describes collider diameter/height/diameter. These are not model dimensions. A later renderer places the model on ground y=0 using position.x/z, existing yaw rotation, and `prop.scale`, then uses the source model rules for kind/variant. The retained `Prop.position`/yaw/scale do not change any collider or gameplay rule. Backdrop geometry is outside this live/destructible body stream and belongs to the later arena slice.

## Validation and bounds

The shared decoder validates descriptor/body-kind combinations before accepting frames. Unknown prop kinds, unknown prop fields, non-integer/out-of-range variants, malformed owners and malformed scales are rejected by serde and/or `valid_frame`. Owner range 0..3 is independent of current human membership: bot projectiles and projectiles still alive after their owner is eliminated remain legal. Descriptor data does not grant input authority.

Existing transport byte limit (256 KiB), 1,024-body bound, finite transform limits, unique safe-integer body IDs and quaternion checks remain in force. No unbounded string is added to the body schema. Integration tests round-trip both arenas and all four owners under the byte limit; exhaustive component tests cover all six kinds, both variants and four source scales including Volcano's cap. Validation tests cover missing/incompatible descriptors, invalid owners/variants/scales/types, unknown fields and oversized body counts.

## Local verification scope

Tests cover source component capture, serialization/deserialization, validation, unchanged collider envelopes, unchanged IDs across repeated frames, removal and monotonic IDs on subsequent spawns. Full native/shared-adapter regressions include seeded simulation parity, rear-contact push, hit/score/pickup/reset/reclaim behavior and fixed explosion geometry. No simulation seeds, blast radii, scoring, collision, reclaim or offline rendering rules are changed.

Retained evidence is under `verification/visual-descriptors/`; operator recovery receipts are under `/home/hermes/.hermes/artifacts/ryggattack/resume-t_927d7833-20261024/`. See the local evidence README for exact attempts, hashes and durations. Interrupted/OOM RED attempts are not passing evidence. Native manual graphics remain deferred; no WASM/browser/public-room/deployment acceptance is claimed. The documented anonymous-bearer theft limitation is unchanged.
