// Read-only classifier of PUBLIC authoritative bodies. Not a mesh/pixel oracle.
// Call per match: network body IDs can be reused by a new simulation.
import assert from 'node:assert/strict';
import {canonical} from './live-deployment-helpers.mjs';
export function assertSharedExplosionEvidence(a, b) {
  // Analyze only frames actually received by BOTH sockets, not unioned events.
  const peer = new Map(b.map(frame => [frame.tick, frame]));
  const shared = a.filter(frame => peer.has(frame.tick));
  for (const frame of shared) assert.deepEqual(canonical(frame), canonical(peer.get(frame.tick)), `observer frame mismatch at tick ${frame.tick}`);
  return {...analyzeExplosionFrames(shared), shared_frames: shared.length, first_tick: shared[0]?.tick ?? null, last_tick: shared.at(-1)?.tick ?? null};
}
const xz = (a, b) => Math.hypot(a[0] - b[0], a[2] - b[2]);
export function analyzeExplosionFrames(frames) {
  const tracks = new Map(), vanished = [];
  let previous, segment = 0;
  for (const frame of frames) {
    assert(Number.isInteger(frame.tick), 'integer authoritative tick');
    if (previous) assert(frame.tick > previous.tick, 'advancing authoritative ticks');
    const contiguous = previous && frame.tick - previous.tick === 4 &&
      frame.simulation_tick - previous.simulation_tick === 4 && !previous.round_finished && !frame.round_finished;
    if (!contiguous) segment++;
    const key = id => `${segment}:${id}`;
    const bodies = new Map(frame.bodies.map(body => [body.id, body]));
    assert.equal(bodies.size, frame.bodies.length, 'unique body IDs in frame');
    if (contiguous) for (const old of previous.bodies) if (!bodies.has(old.id)) {
      if ([0, 3].includes(old.kind)) vanished.push({body: old, tick: frame.tick, segment});
      if (old.kind === 4 && tracks.has(key(old.id))) tracks.get(key(old.id)).removed_tick = frame.tick;
    }
    for (const body of frame.bodies.filter(body => body.kind === 4)) {
      assert(body.scale.length === 3 && body.scale.every(n => Number.isFinite(n) && n > 0), 'positive finite kind-4 wire scale');
      assert(body.scale.every(n => Math.abs(n - body.scale[0]) < 1e-5), 'isotropic fireball scale');
      assert(body.position.length === 3 && body.position.every(Number.isFinite), 'finite explosion position');
      const track = tracks.get(key(body.id)) ?? {id: body.id, segment, position: body.position, first_tick: frame.tick, max_scale: 0, samples: 0};
      assert(!track.removed_tick, 'explosion ID must not reappear within a match');
      assert(xz(track.position, body.position) < 1e-4, 'explosion centre stable');
      track.max_scale = Math.max(track.max_scale, body.scale[0]); track.samples++;
      tracks.set(key(body.id), track);
    }
    previous = frame;
  }
  const explosions = [...tracks.values()];
  const primaries = explosions.filter(e => e.max_scale >= 1.4 && e.samples >= 2 && e.removed_tick &&
    vanished.some(v => v.segment === e.segment && v.body.kind === 0 && Math.abs(v.tick - e.first_tick) <= 4 && xz(v.body.position, e.position) < .85));
  const wall = primaries.find(e => Math.abs(Math.max(Math.abs(e.position[0]), Math.abs(e.position[2])) - 9) < .001);
  let chain;
  for (const primary of primaries) {
    for (const prop of vanished.filter(v => v.segment === primary.segment && v.body.kind === 3 && Math.abs(v.tick - primary.first_tick) <= 4 &&
      xz(v.body.position, primary.position) <= 2 + v.body.scale[0] / 2)) {
      const secondary = explosions.find(e => e.segment === primary.segment && e.id !== primary.id && e.first_tick > prop.tick && e.first_tick <= prop.tick + 32 &&
        e.samples >= 2 && e.removed_tick && e.max_scale >= .65 && e.max_scale <= .9 &&
        xz(e.position, prop.body.position) < .01);
      if (secondary) {
        chain = {primary_id: primary.id, primary_max_scale: primary.max_scale, primary_removed_tick: primary.removed_tick,
          prop_id: prop.body.id, prop_removed_tick: prop.tick, chain_id: secondary.id, chain_first_tick: secondary.first_tick,
          chain_max_scale: secondary.max_scale, chain_removed_tick: secondary.removed_tick};
        break;
      }
    }
    if (chain) break;
  }
  return {complete: Boolean(wall && chain), wall: wall ? {primary_id: wall.id, position: wall.position, max_scale: wall.max_scale, removed_tick: wall.removed_tick} : null,
    chain: chain ?? null, explosion_ids: explosions, projectile_removals: vanished.filter(v => v.body.kind === 0).length,
    prop_removals: vanished.filter(v => v.body.kind === 3).length,
    scope: 'Conservative spatial/temporal wire correlation, not protocol causal tags, pixel dimensions, or runtime mesh measurements.'};
}
