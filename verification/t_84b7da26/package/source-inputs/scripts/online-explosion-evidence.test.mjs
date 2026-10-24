import test from 'node:test';
import assert from 'node:assert/strict';
const load = async () => {
  const module = await import('./online-explosion-evidence.mjs').catch(() => ({}));
  assert.equal(typeof module.analyzeExplosionFrames, 'function', 'explosion evidence analyzer must exist');
  return module;
};
const body = (id, kind, x, scale = 1, z = 0) => ({id, kind, position: [x, .5, z], rotation: [0, 0, 0, 1], scale: [scale, scale, scale]});
const frame = (tick, bodies, round_finished = false) => ({tick, simulation_tick: tick, bodies, round_finished});
// Synthetic protocol snapshots test only the conservative evidence classifier;
// these are NEVER accepted as natural gameplay or renderer/mesh proof.
const fixture = () => [
  frame(4, [body(1, 0, 8.6), body(2, 0, 0), body(3, 3, .4)]),
  frame(8, [body(10, 4, 9, .6), body(11, 4, 0, .5)]),
  frame(12, [body(10, 4, 9, 1.55), body(11, 4, 0, 1.55), body(12, 4, .4, .4)]),
  frame(16, [body(12, 4, .4, .86)]),
  frame(20, []),
];
test('wire evidence requires primary, wall, destroyed prop and smaller distinct delayed chain IDs', async () => {
  const {analyzeExplosionFrames} = await load();
  const result = analyzeExplosionFrames(fixture());
  assert.equal(result.complete, true);
  assert.equal(result.wall.primary_id, 10);
  assert.equal(result.chain.primary_id, 11);
  assert.equal(result.chain.prop_id, 3);
  assert.equal(result.chain.chain_id, 12);
  assert.equal(result.chain.primary_removed_tick, 16);
  assert.equal(result.chain.chain_removed_tick, 20);
});
test('missing shots, gaps, round clears, unrelated prop positions and tiny-only blasts never pass', async () => {
  const {analyzeExplosionFrames} = await load();
  const noShots = fixture().map(f => ({...f, bodies: f.bodies.filter(b => b.kind !== 0)}));
  assert.equal(analyzeExplosionFrames(noShots).complete, false);
  assert.equal(analyzeExplosionFrames(fixture().filter(f => f.tick !== 8)).complete, false);
  assert.equal(analyzeExplosionFrames(fixture().map(f => ({...f, round_finished: true}))).complete, false);
  const unrelated = fixture(); unrelated[0].bodies.find(b => b.kind === 3).position[0] = 5;
  assert.equal(analyzeExplosionFrames(unrelated).complete, false);
  const tiny = fixture(); tiny.forEach(f => f.bodies.filter(b => b.kind === 4).forEach(b => b.scale = b.scale.map(n => n * .16)));
  assert.equal(analyzeExplosionFrames(tiny).complete, false);
});
test('exact legal 4,8,32,36,40 gap cannot complete a causal chain', async () => {
  const {analyzeExplosionFrames} = await load();
  const frames = fixture().map((f, i) => ({...f, tick: [4, 8, 32, 36, 40][i], simulation_tick: [4, 8, 32, 36, 40][i]}));
  assert.equal(analyzeExplosionFrames(frames).complete, false);
});
test('round-finished boundary cannot connect an unfinished causal chain', async () => {
  const {analyzeExplosionFrames} = await load();
  const frames = fixture(); frames[2].round_finished = true;
  assert.equal(analyzeExplosionFrames(frames).complete, false);
});
test('completed historical proof survives a later sampling gap or round finish', async () => {
  const {analyzeExplosionFrames} = await load();
  for (const tail of [frame(44, []), frame(24, [], true)]) {
    assert.equal(analyzeExplosionFrames([...fixture(), tail]).complete, true);
  }
});
test('disjoint observer ticks cannot supply shared evidence', async () => {
  const {assertSharedExplosionEvidence} = await load();
  assert.equal(assertSharedExplosionEvidence(fixture(), fixture().map(f => ({...f, tick: f.tick + 100}))).complete, false);
});
test('nonpositive scales, duplicate IDs and regressing ticks reject corrupt evidence', async () => {
  const {analyzeExplosionFrames} = await load();
  assert.throws(() => analyzeExplosionFrames([frame(4, [body(1, 4, 0, 0)])]), /positive/);
  assert.throws(() => analyzeExplosionFrames([frame(4, [body(1, 4, 0), body(1, 4, 0)])]), /unique/);
  assert.throws(() => analyzeExplosionFrames([frame(8, []), frame(4, [])]), /advancing/);
});
test('shared observer gate rejects a disagreement even at an explosion-free tick', async () => {
  const module = await load();
  assert.equal(typeof module.assertSharedExplosionEvidence, 'function', 'shared observer gate must exist');
  const a = fixture(), b = fixture();
  assert.equal(module.assertSharedExplosionEvidence(a, b).complete, true);
  b[0].bodies[0].position[0] += .1;
  assert.throws(() => module.assertSharedExplosionEvidence(a, b), /observer frame mismatch/);
  assert.equal(module.assertSharedExplosionEvidence(fixture(), fixture().filter(f => f.tick !== 8)).complete, false, 'unioned single-observer evidence is forbidden');
  assert.equal(module.assertSharedExplosionEvidence([], []).complete, false, 'empty observers cannot pass');
});
