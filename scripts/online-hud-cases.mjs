import assert from 'node:assert/strict';

// HARNESS policy only. Both cases use the existing ordinary game rules/deadlines.
export const MATCH_CASES = Object.freeze([
  Object.freeze({id: 'preflight', target: 7, inputChecks: true, completionMs: 180000}),
  Object.freeze({id: 'natural', target: 1, inputChecks: false, completionMs: 180000})
]);

export function createMatchCase(plan, atMs) {
  return {plan, states: new Map(), latest: null, winner: null, lobbies: [],
    domTransition: createOutcomeTransition(),
    stats: {case_id: plan.id, target: plan.target, configured_at_ms: atMs,
      scope: plan.inputChecks ? 'Real focus/menu/diagnostic input-window preflight; naturally completed target-seven match, NOT target-one core proof.' : 'Original target-one natural-completion core; no slow UI prechecks.',
      samples: 0, matched_frames: 0, dom_matches: 0, max_published_tick_lag: 0,
      checks: [], outcomes: [], out: [], buffs: [], input: {count: 0, neutral_fire: 0}}};
}

export function caseForSample(cases, sample) {
  if (sample.case_id === null) return null;
  assert(cases.has(sample.case_id), 'unknown match case');
  return cases.get(sample.case_id);
}

export function caseAtPageTime(cases, pageMs) {
  assert(Number.isFinite(pageMs), 'finite page observation timestamp required');
  return [...cases.values()].reverse().find(match =>
    Number.isFinite(match.observed_from_page_ms) && pageMs >= match.observed_from_page_ms) ?? null;
}

// Acceptance-oracle correction: wire Lobby does not instantly pump the client.
// Only strictly pre-wire observations prove the result; never truncate evidence.
export function isLobbyClear(sample, roomId) {
  let serialized;
  try { serialized = JSON.parse(sample.raw); } catch { return false; }
  return sample.mounted === true && serialized?.lobby?.id === roomId &&
    serialized.lobby.phase === 'Lobby' && serialized.frame === null &&
    JSON.stringify(serialized) === JSON.stringify(sample.payload) &&
    sample.names?.length === 0 && sample.states?.length === 0 && sample.outcome === '' &&
    sample.player_count === 0 && sample.outcome_hidden === true && sample.buffs_absent === true;
}

// Case-local observation clock, NOT server-send latency or a cross-clock delta.
// The bridge polls at 100ms; the first observed post-Running raw Lobby starts
// this bound. Initial/configuration Lobby must never arm the return transition.
export function createOutcomeTransition() {
  return {clock_semantics: 'Page performance.now(): first observed raw Lobby AFTER observed Running to actual serialized + DOM clear; not server-send latency. Node receipt clock independently bounds wire Lobby to clear.',
    observed_running_page_ms: null, first_raw_lobby_page_ms: null,
    clear_page_ms: null, clear_delay_page_ms: null, last_page_ms: null,
    sample_count: 0, transition_sample_count: 0, old_outcome_count: 0};
}

// Shared by live validation and final replay: no separate instantaneous guard.
export function observeOutcomeTransition(state, sample, roomId, caseId) {
  assert.equal(sample.case_id, caseId, 'case-local DOM transition observations required');
  assert(Number.isFinite(sample.page_ms), 'finite page observation timestamp required');
  assert(state.last_page_ms === null || sample.page_ms >= state.last_page_ms,
    'DOM transition page observations must be ordered');
  state.last_page_ms = sample.page_ms;
  state.sample_count++;
  const lobby = sample.payload?.lobby;
  if (lobby) assert.equal(lobby.id, roomId, 'case room required for DOM clear transition');
  if (state.clear_page_ms !== null) {
    assert(!sample.outcome, 'post-clear outcome reappearance within this case');
    return state;
  }
  if (lobby?.phase === 'Running' && state.observed_running_page_ms === null)
    state.observed_running_page_ms = sample.page_ms;
  if (state.observed_running_page_ms === null) {
    if (lobby?.phase === 'Lobby') assert(!sample.outcome, 'initial Lobby cannot retain outcome before observed Running');
    return state;
  }
  if (lobby?.phase === 'Lobby' && state.first_raw_lobby_page_ms === null)
    state.first_raw_lobby_page_ms = sample.page_ms;
  if (state.first_raw_lobby_page_ms === null) return state;
  state.transition_sample_count++;
  const delay = sample.page_ms - state.first_raw_lobby_page_ms;
  assert(delay <= 100, 'actual serialized + DOM clear exceeds observed raw-Lobby 100ms poll bound');
  if (sample.outcome) state.old_outcome_count++;
  if (isLobbyClear(sample, roomId)) {
    state.clear_page_ms = sample.page_ms;
    state.clear_delay_page_ms = delay;
  }
  return state;
}

export function finishOutcomeTransition(state) {
  assert(state.observed_running_page_ms !== null && state.first_raw_lobby_page_ms !== null &&
    state.clear_page_ms !== null, 'actual serialized + DOM clear required after case-local observed Running then raw Lobby');
  return state;
}

export function assertOutcomeTimeline({returned, named, samples, roomId, caseId, expectedOutcome}) {
  assert(returned?.case_id === caseId && returned.phase === 'Lobby', 'case-local authoritative Lobby required');
  assert(Number.isFinite(returned.at_ms), 'finite return timestamp required');
  for (const sample of [...named, ...samples]) {
    assert.equal(sample.case_id, caseId, 'case-local observations required; no cross-case borrowing');
    assert(Number.isFinite(sample.at_ms) && Number.isFinite(sample.page_ms), 'finite observation timestamp required');
  }
  assert(named.every(s => s.phase === 'Running' && s.outcome === expectedOutcome),
    'named winner DOM proof must match the authoritative winner and client Running');
  const before = named.filter(s => s.at_ms < returned.at_ms);
  const tail = named.filter(s => s.at_ms >= returned.at_ms);
  const span = rows => rows.length < 2 ? 0 : Math.max(...rows.map(s => s.page_ms)) - Math.min(...rows.map(s => s.page_ms));
  assert(before.length >= 2 && span(before) >= 100,
    'at least two pre-return named match DOM observations must persist >=100ms');
  assert(samples.every((s, i) => i === 0 || s.at_ms >= samples[i - 1].at_ms), 'clear samples must be ordered');
  const domTransition = createOutcomeTransition();
  for (const sample of samples) observeOutcomeTransition(domTransition, sample, roomId, caseId);
  finishOutcomeTransition(domTransition);
  const firstClear = samples.find(s => s.page_ms === domTransition.clear_page_ms &&
    s.at_ms >= returned.at_ms && isLobbyClear(s, roomId));
  assert(firstClear && firstClear.at_ms - returned.at_ms <= 1000,
    'actual DOM and serialized HUD clear must be observed within 1000ms of wire Lobby');
  assert(tail.every(s => s.at_ms <= firstClear.at_ms), 'Running outcome tail must end by the bounded clear observation');
  return {before_count: before.length, before_persistence_ms: span(before),
    running_tail_count: tail.length, running_tail_persistence_ms: span(tail),
    first_clear: firstClear, clear_delay_ms: firstClear.at_ms - returned.at_ms,
    dom_transition: domTransition};
}

export function assertFreshReset(frame) {
  assert(frame && frame.winner === null && frame.round_finished === false &&
    frame.wins.every(wins => wins === 0) && frame.carts.every(cart =>
      cart.speed_seconds === 0 && cart.all_hits_seconds === 0),
  'fresh match reset must have zero wins, no winner, unfinished round and no old buffs');
}
