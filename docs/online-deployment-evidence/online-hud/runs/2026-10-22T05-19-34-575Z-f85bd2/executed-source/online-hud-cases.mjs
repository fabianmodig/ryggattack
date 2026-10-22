import assert from 'node:assert/strict';

// HARNESS policy only. Both cases use the existing ordinary game rules/deadlines.
export const MATCH_CASES = Object.freeze([
  Object.freeze({id: 'preflight', target: 7, inputChecks: true, completionMs: 180000}),
  Object.freeze({id: 'natural', target: 1, inputChecks: false, completionMs: 180000})
]);

export function createMatchCase(plan, atMs) {
  return {plan, states: new Map(), latest: null, winner: null, lobbies: [],
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

export function assertFreshReset(frame) {
  assert(frame && frame.winner === null && frame.round_finished === false &&
    frame.wins.every(wins => wins === 0) && frame.carts.every(cart =>
      cart.speed_seconds === 0 && cart.all_hits_seconds === 0),
  'fresh match reset must have zero wins, no winner, unfinished round and no old buffs');
}
