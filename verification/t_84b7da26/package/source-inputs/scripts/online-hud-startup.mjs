// Pure wire-order policy. The only exception to case-scoped Input is the
// initial Welcome-to-first-Lobby handshake gap, before any Case or Configure.
// No timer/deadline is relaxed and no packet is discarded by this policy.
export function startupInputPolicy(previous, event) {
  const state = {...(previous ?? {hello_count: 0, welcome_count: 0, admitted: false,
    closed: false, input_count: 0, startup_count: 0})};
  if (event.kind === 'Hello') {
    state.hello_count++;
    state.hello_at_ms ??= event.at_ms ?? null;
  }
  if (event.kind === 'Welcome') {
    state.welcome_count++;
    state.welcome_at_ms ??= event.at_ms ?? null;
    const id = event.identity;
    state.admitted = state.hello_count === 1 && state.welcome_count === 1 &&
      ['version', 'room', 'participant', 'seat'].every(key => Number.isInteger(id?.[key]) && id[key] >= 0);
  }
  if (['Case', 'Configure', 'Lobby', 'Running'].includes(event.kind)) {
    state.closed = true;
    state.closed_by ??= {kind: event.kind, at_ms: event.at_ms ?? null};
  }
  if (event.kind !== 'Input') return {state, classification: null};
  state.input_count++;
  if (event.case_id != null) return {state, classification: 'case-input'};
  const input = event.input;
  if (!state.closed && state.admitted && state.hello_count === 1 && state.welcome_count === 1 &&
      state.input_count === 1 && state.startup_count === 0 &&
      input?.sequence === 1 && input.horizontal === 0 && input.vertical === 0 && input.fire === false) {
    state.startup_count++;
    state.startup_at_ms = event.at_ms ?? null;
    return {state, classification: 'startup-neutral'};
  }
  return {state, classification: 'rejected', reason: 'Input belongs to an active case or the single neutral startup handshake'};
}
