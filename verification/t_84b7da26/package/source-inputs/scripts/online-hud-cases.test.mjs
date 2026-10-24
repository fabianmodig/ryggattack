import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
// A missing helper is a normal assertion failure during the initial RED run.
const api = await import('./online-hud-cases.mjs').catch(error => {
  if (error.code === 'ERR_MODULE_NOT_FOUND') return {};
  throw error;
});
const source = fs.readFileSync(new URL('./online-hud-smoke.mjs', import.meta.url), 'utf8');

test('ordinary same-room preflight precedes the target-one core, each capped at 180s', () => {
  assert.deepEqual(api.MATCH_CASES, [
    {id: 'preflight', target: 7, inputChecks: true, completionMs: 180000},
    {id: 'natural', target: 1, inputChecks: false, completionMs: 180000}
  ]);
  assert.match(source, /for \(const plan of MATCH_CASES\)/);
  assert.match(source, /if \(plan.inputChecks\) await runInputPreflight\(\)/);
});

test('new core cannot inherit preflight winner, state ticks, outcomes, inputs or lobby history', () => {
  assert.equal(typeof api.createMatchCase, 'function');
  const preflight = api.createMatchCase(api.MATCH_CASES[0], 100);
  preflight.states.set(10, {tick: 10, winner: 1}); preflight.winner = {tick: 10};
  preflight.stats.outcomes.push({outcome: 'Bot 2 wins the match'});
  preflight.stats.input.count++; preflight.lobbies.push({phase: 'Lobby'});
  const core = api.createMatchCase(api.MATCH_CASES[1], 200);
  assert.equal(core.winner, null); assert.equal(core.latest, null);
  assert.equal(core.states.size, 0); assert.deepEqual(core.stats.outcomes, []);
  assert.deepEqual(core.stats.input, {count: 0, neutral_fire: 0});
  assert.deepEqual(core.lobbies, []); assert.notEqual(core.stats, preflight.stats);
});

test('queued preflight observations remain assigned to preflight after core starts', () => {
  assert.equal(typeof api.caseForSample, 'function');
  const cases = new Map(api.MATCH_CASES.map(p => [p.id, api.createMatchCase(p, 0)]));
  assert.equal(api.caseForSample(cases, {case_id: 'preflight'}), cases.get('preflight'));
  assert.equal(api.caseForSample(cases, {case_id: 'natural'}), cases.get('natural'));
  assert.equal(api.caseForSample(cases, {case_id: null}), null);
  assert.throws(() => api.caseForSample(cases, {case_id: 'unknown'}), /unknown match case/);
});

test('fresh reset rejects inherited winner, wins, timer buffs and finished rounds', () => {
  assert.equal(typeof api.assertFreshReset, 'function');
  const frame = {tick: 2, winner: null, round_finished: false, wins: [0, 0], carts: [{speed_seconds: 0, all_hits_seconds: 0}]};
  assert.doesNotThrow(() => api.assertFreshReset(frame));
  for (const change of [{winner: 1}, {wins: [0, 7]}, {round_finished: true},
    {carts: [{speed_seconds: 1, all_hits_seconds: 0}]}, {carts: [{speed_seconds: 0, all_hits_seconds: 1}]}]) {
    assert.throws(() => api.assertFreshReset({...frame, ...change}), /fresh match/);
  }
});

test('delayed binding delivery uses page observation time, not the currently active case', () => {
  assert.equal(typeof api.caseAtPageTime, 'function');
  const preflight = api.createMatchCase(api.MATCH_CASES[0], 100);
  const core = api.createMatchCase(api.MATCH_CASES[1], 1000);
  preflight.observed_from_page_ms = 200; core.observed_from_page_ms = 2000;
  const cases = new Map([['preflight', preflight], ['natural', core]]);
  assert.equal(api.caseAtPageTime(cases, 199), null);
  assert.equal(api.caseAtPageTime(cases, 1999), preflight);
  assert.equal(api.caseAtPageTime(cases, 2000), core);
  assert.throws(() => api.caseAtPageTime(cases, NaN), /observation timestamp/);
});

test('input windows, screenshot and publication-lag limits remain intact', () => {
  assert.match(source, /from \+ 3/); assert.match(source, /input samples`, 5000/);
  assert.match(source, /lag <= 64/); assert.match(source, /timeout: 60000/);
  assert.match(source, /\['Hello', 'Configure', 'Ready', 'Configure', 'Ready'\]/);
});
