import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {canonical} from './live-deployment-helpers.mjs';
const source = fs.readFileSync(new URL('./online-hud-smoke.mjs', import.meta.url), 'utf8');
const liveBody = source.slice(source.indexOf('function validateSample('), source.indexOf('async function drainSamples()'));

test('live completion drains actual clear samples before the temporal oracle and reports separate proof/tail', () => {
  const body = source.slice(source.indexOf('async function completeMatch()'), source.indexOf('async function leaveAndCloseGame()'));
  assert.match(source, /player_count: root\?\.querySelectorAll/);
  assert.match(source, /outcome_hidden:/); assert.match(source, /buffs_absent:/);
  assert.match(body, /isLobbyClear\(s, identity.room\)/);
  assert.match(body, /s\.page_ms === activeCase\.domTransition\.clear_page_ms/);
  assert.match(body, /await drainSamples\(\);[\s\S]*assertOutcomeTimeline\(/);
  assert.match(body, /stats\.outcome_timeline =/);
  assert.match(body, /stats\.outcome_persistence_ms = timeline\.before_persistence_ms/);
  assert.doesNotMatch(body, /named\.every\(s => s.at_ms < returned.at_ms\)/);
  assert.doesNotMatch(body, /waitForFunction/);
});

test('live validation shares the case-local bounded transition rule, including capture-time drains', () => {
  assert.match(liveBody, /observeOutcomeTransition\(match\.domTransition, sample, identity.room, match.plan.id\)/);
  assert.match(liveBody, /stats\.dom_transition = match\.domTransition/);
  assert.doesNotMatch(liveBody, /client Lobby must never|no-transition-grace/);
  assert(liveBody.indexOf('observeOutcomeTransition(') < liveBody.indexOf('if (!p || !sample.mounted)'));
});

test('failed validation still publishes all captured and pending observations without trimming tails', () => {
  assert.match(source, /redact\(\[\.\.\.capturedSamples, \.\.\.pendingSamples\], secrets\)/);
});

const api = await import('./online-hud-cases.mjs');
const outcome = 'Bot 2 wins the match';
const human = {id: 'participant', seat: 0};
const payloadFor = phase => ({lobby: {id: 'room', phase}, frame: null,
  menu: false, notice: null, rtt_ms: 0, seat: 0, status: 'Connected'});
const clear = at_ms => {
  const payload = payloadFor('Lobby');
  return {case_id: 'preflight', at_ms, page_ms: at_ms + 500, payload,
    raw: JSON.stringify(payload), mounted: true, visible: true,
    room: {lobby: {id: 'room', phase: 'Lobby', humans: [human]}, participant: human.id},
    names: [], states: [], outcome: '', player_count: 0, outcome_hidden: true, buffs_absent: true};
};
const running = at_ms => {
  const payload = payloadFor('Running');
  return {...clear(at_ms), phase: 'Running', payload, raw: JSON.stringify(payload), outcome,
    room: {lobby: {id: 'room', phase: 'Running', humans: [human]}, participant: human.id}};
};
const stale = at_ms => ({...clear(at_ms), names: ['Bot 2'], states: ['7 wins · IN'],
  player_count: 2, outcome_hidden: false, outcome});
const evaluate = (overrides = {}) => {
  assert.equal(typeof api.assertOutcomeTimeline, 'function', 'temporal acceptance helper exists');
  const options = {caseId: 'preflight', roomId: 'room', expectedOutcome: outcome,
    returned: {case_id: 'preflight', at_ms: 63129, phase: 'Lobby'},
    named: [...Array.from({length: 42}, (_, i) => running(60641 + Math.floor(i * 2475 / 41))),
      running(63166), running(63266)], samples: [clear(63300)], ...overrides};
  return api.assertOutcomeTimeline({...options, samples: [running(60000), ...options.samples]});
};

// Exercise the actual live validator body without importing the smoke CLI (which
// starts a browser/backend). The real shared transition rule and case store run;
// frame projection is not involved in these raw-Lobby/DOM timing sequences.
const liveValidator = match => new Function('assert', 'caseForSample', 'observeOutcomeTransition',
  'identity', 'cases', 'report', 'same', 'canonical', 'model', `${liveBody}; return validateSample;`)(
  assert, api.caseForSample, api.observeOutcomeTransition,
  {room: 'room', participant: human.id, seat: 0}, new Map([[match.plan.id, match]]), {},
  assert.deepEqual, canonical, () => ({names: [], states: [], outcome: ''}));
const runLive = samples => {
  const match = api.createMatchCase(api.MATCH_CASES[0], 0);
  match.lobbies = ['Lobby', 'Running'].map(phase => ({id: 'room', phase, humans: [human]}));
  const validate = liveValidator(match);
  for (const sample of [running(60000), ...samples]) validate(sample);
  api.finishOutcomeTransition(match.domTransition);
  return match;
};

// Every bounded-transition scenario must agree through BOTH the actual live
// validator and the final timeline replay, not merely call different guards.
const bothPass = samples => {
  const result = evaluate({samples});
  const live = runLive(samples);
  assert.deepEqual(live.domTransition, result.dom_transition);
  assert.strictEqual(live.stats.dom_transition, live.domTransition);
  return result;
};
const bothFail = (samples, pattern) => {
  assert.throws(() => evaluate({samples}), pattern);
  assert.throws(() => runLive(samples), pattern);
};

test('all-after, one-before, and short-before persistence cannot borrow the tail', () => {
  for (const named of [[running(63130), running(63300)],
    [running(63100), running(63300)],
    [running(63000), running(63099), running(63300)]]) {
    assert.throws(() => evaluate({named}), /pre-return/);
  }
  assert.doesNotThrow(() => evaluate({named: [running(63000), running(63100)]}));
});

test('clear must be actual serialized HUD plus empty DOM, first observed within 1000ms of wire Lobby', () => {
  assert.doesNotThrow(() => evaluate({samples: [clear(64129)]}));
  for (const samples of [[], [clear(64130)], [clear(63128)],
    [clear(64130), clear(63300)]]) {
    assert.throws(() => evaluate({samples}), /clear|ordered/);
  }
  for (const change of [{mounted: false}, {names: ['Bot 2']}, {states: ['OUT']},
    {outcome}, {player_count: 1}, {outcome_hidden: false}, {buffs_absent: false},
    {raw: '{}'}, {raw: 'bad json'},
    {payload: {lobby: {id: 'room', phase: 'Running'}, frame: null}},
    {payload: {lobby: {id: 'room', phase: 'Lobby'}, frame: {winner: 1}}}]) {
    assert.throws(() => evaluate({samples: [{...clear(63300), ...change}]}), /clear|serialized/);
  }
  assert.equal(evaluate({samples: [clear(63400), clear(63500)]}).first_clear.at_ms, 63400);
});

test('case-local proof rejects foreign outcomes, clear samples, and return boundaries', () => {
  for (const overrides of [
    {named: [running(63000), {...running(63100), case_id: 'natural'}]},
    {samples: [{...clear(63300), case_id: 'natural'}]},
    {returned: {case_id: 'natural', phase: 'Lobby', at_ms: 63129}},
    {samples: [clear(63300)].map(s => ({...s, payload: {lobby: {id: 'other', phase: 'Lobby'}, frame: null}}))}
  ]) assert.throws(() => evaluate(overrides), /case|clear/);
});

test('named proof must still match the authoritative winner and client Running', () => {
  for (const change of [{phase: 'Lobby'}, {phase: 'Countdown'}, {outcome: 'Other wins the match'},
    {at_ms: NaN}, {page_ms: NaN}]) {
    assert.throws(() => evaluate({named: [running(63000), {...running(63100), ...change}]}), /Running|named|timestamp/);
  }
  assert.throws(() => evaluate({named: [running(63000), running(63100), running(64200)]}), /tail/);
});

test('raw Lobby may retain old DOM until the observed 100ms poll bound, then actually clears', () => {
  for (const delay of [0, 50, 100]) {
    const result = bothPass([stale(63300), ...(delay ? [stale(63300 + delay / 2)] : []), clear(63300 + delay)]);
    assert.equal(result.dom_transition.first_raw_lobby_page_ms, 63800);
    assert.equal(result.dom_transition.clear_delay_page_ms, delay);
    assert.equal(result.dom_transition.old_outcome_count, delay ? 2 : 1);
    assert.equal(result.dom_transition.transition_sample_count, delay ? 3 : 2);
    assert.match(result.dom_transition.clock_semantics, /Page performance.now.*not server-send latency/);
  }
});

test('historical async clock sequence is reproducible without relabelling old incomplete evidence', () => {
  // Synthetic regression carrying the historical clock order; these explicit
  // clear flags are NEW test inputs, not flags present in the failed old run.
  const samples = [stale(63290), {...clear(63341), page_ms: 63290 + 500 + 50}];
  const result = bothPass(samples);
  assert.equal(result.clear_delay_ms, 212);
  assert.equal(result.dom_transition.clear_delay_page_ms, 50);
  const incompleteOldShape = {...samples[1]};
  delete incompleteOldShape.player_count;
  delete incompleteOldShape.outcome_hidden;
  delete incompleteOldShape.buffs_absent;
  bothFail([samples[0], incompleteOldShape], /actual serialized.*clear required/);
});

test('over-100ms stale DOM and late actual clear fail even when wire-to-clear is under 1000ms', () => {
  bothFail([stale(63300), stale(63401), clear(63402)], /100ms poll bound/);
  bothFail([stale(63300), clear(63401)], /100ms poll bound/);
});

test('missing actual clear fails even with empty outcome or observations ending at the bound', () => {
  bothFail([stale(63300), stale(63400)], /actual serialized.*clear required/);
  bothFail([stale(63300), {...clear(63400), outcome_hidden: false}], /actual serialized.*clear required/);
  bothFail([stale(63300), {...clear(63400), names: ['Bot 2']}], /actual serialized.*clear required/);
});

test('post-clear outcome reappearance fails in any phase and even inside the original grace', () => {
  for (const sample of [stale(63375), running(63375), {...stale(63375), payload: null}])
    bothFail([stale(63300), clear(63350), sample], /post-clear outcome reappearance/);
});

test('initial configuration Lobby does not arm return or poison a later natural winner', () => {
  const samples = [clear(60500), running(60600), running(63100), stale(63300), clear(63400)];
  const match = api.createMatchCase(api.MATCH_CASES[0], 0);
  match.lobbies = ['Lobby', 'Running'].map(phase => ({id: 'room', phase, humans: [human]}));
  const validate = liveValidator(match);
  for (const sample of samples) validate(sample);
  const result = api.assertOutcomeTimeline({caseId: 'preflight', roomId: 'room', expectedOutcome: outcome,
    returned: {case_id: 'preflight', phase: 'Lobby', at_ms: 63129},
    named: [running(60600), running(63100)], samples});
  assert.deepEqual(match.domTransition, result.dom_transition);
  assert.equal(match.domTransition.observed_running_page_ms, 61100);
  assert.equal(match.domTransition.first_raw_lobby_page_ms, 63800);
});

test('new core case has fresh transition state and cannot borrow or inherit preflight clear', () => {
  const first = runLive([stale(63300), clear(63350)]);
  const next = api.createMatchCase(api.MATCH_CASES[1], 70000);
  assert.notStrictEqual(next.domTransition, first.domTransition);
  assert.equal(next.domTransition.first_raw_lobby_page_ms, null);
  assert.equal(next.domTransition.clear_page_ms, null);
  const observe = s => api.observeOutcomeTransition(next.domTransition, {...s, case_id: 'natural'}, 'room', 'natural');
  observe(clear(70000)); observe(running(71000));
  assert.equal(next.domTransition.clear_page_ms, null);
  assert.throws(() => api.finishOutcomeTransition(next.domTransition), /clear required/);
  observe(stale(73000)); observe(clear(73100));
  assert.equal(api.finishOutcomeTransition(next.domTransition).clear_delay_page_ms, 100);
  assert.throws(() => api.observeOutcomeTransition(next.domTransition, stale(73101), 'room', 'natural'), /case-local/);
});

test('page clock, not delayed Node receipt, controls the independently bounded DOM transition', () => {
  bothFail([stale(63300), {...clear(63350), page_ms: 63901}], /100ms poll bound/);
  const result = bothPass([stale(63300), {...clear(64000), page_ms: 63850}]);
  assert.equal(result.dom_transition.clear_delay_page_ms, 50);
  assert.equal(result.clear_delay_ms, 871);
  assert.throws(() => evaluate({samples: [stale(63300), {...clear(64130), page_ms: 63850}]}), /within 1000ms/);
});

test('initial Lobby cannot carry an outcome before this case has observed Running', () => {
  const state = api.createOutcomeTransition();
  assert.throws(() => api.observeOutcomeTransition(state, stale(59000), 'room', 'preflight'), /initial Lobby/);
});

test('published room scope remains enforced after the clear observation', () => {
  bothFail([stale(63300), clear(63350), {...clear(63400),
    payload: {...clear(63400).payload, lobby: {id: 'other', phase: 'Lobby'}}}], /case room/);
});

test('initial Lobby with delayed receipt cannot be selected as the returned-match clear', () => {
  const samples = [{...clear(63130), page_ms: 100}, {...running(63140), page_ms: 200},
    {...stale(63290), page_ms: 300}, {...clear(63341), page_ms: 350}];
  const result = api.assertOutcomeTimeline({caseId: 'preflight', roomId: 'room', expectedOutcome: outcome,
    returned: {case_id: 'preflight', phase: 'Lobby', at_ms: 63129},
    named: [running(63000), running(63100)], samples});
  assert.equal(result.first_clear.at_ms, 63341);
  assert.equal(result.clear_delay_ms, 212);
});

test('42 before-return observations prove 2475ms; two Running in-transit tail samples are separate', () => {
  const {dom_transition, ...result} = evaluate();
  assert.deepEqual(result, {before_count: 42, before_persistence_ms: 2475,
    running_tail_count: 2, running_tail_persistence_ms: 100,
    first_clear: clear(63300), clear_delay_ms: 171});
  assert.equal(dom_transition.clear_delay_page_ms, 0);
});
