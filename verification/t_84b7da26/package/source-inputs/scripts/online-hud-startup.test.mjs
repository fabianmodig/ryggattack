import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import {EventEmitter} from 'node:events';
const api = await import('./online-hud-startup.mjs').catch(error => {
  if (error.code === 'ERR_MODULE_NOT_FOUND') return {};
  throw error;
});

// Packet and observation times from the immutable failed run
// 2026-10-22T05-48-27-252Z-673520. Welcome must be observed between Hello
// and Input; the old report did not timestamp Welcome, so no time is invented.
const packet = {sequence: 1, horizontal: 0, vertical: 0, fire: false};
const identity = {version: 3, room: 3, participant: 2, seat: 0};
function step(state, event) { return api.startupInputPolicy(state, event); }
function handshake() {
  let state = step(undefined, {kind: 'Hello', at_ms: 8230}).state;
  return step(state, {kind: 'Welcome', identity}).state;
}

test('actual wire-order sequence-one neutral is startup evidence, never case input proof', () => {
  assert.equal(typeof api.startupInputPolicy, 'function');
  const before = handshake();
  const snapshot = structuredClone(before);
  const result = step(before, {kind: 'Input', input: packet, at_ms: 8257, case_id: null});
  assert.equal(result.classification, 'startup-neutral');
  assert.deepEqual(before, snapshot, 'policy is pure');
  assert.equal(result.state.startup_count, 1);
  assert.equal(result.state.hello_at_ms, 8230);
  assert.equal(result.state.startup_at_ms, 8257);
  let state = step(result.state, {kind: 'Lobby', phase: 'Lobby', at_ms: 8262}).state;
  state = step(state, {kind: 'Case', case_id: 'preflight', at_ms: 8300}).state;
  state = step(state, {kind: 'Configure', at_ms: 8420}).state;
  assert.deepEqual(state.closed_by, {kind: 'Lobby', at_ms: 8262});
  assert.equal(step(state, {kind: 'Input', input: {...packet, sequence: 2}, case_id: 'preflight'}).classification, 'case-input');
});

for (const [name, change] of Object.entries({fire: {fire: true}, horizontal: {horizontal: 1}, vertical: {vertical: -1},
  'missing fire': {fire: undefined}, 'missing horizontal': {horizontal: undefined}, 'wrong sequence': {sequence: 2},
  'zero sequence': {sequence: 0}, 'string sequence': {sequence: '1'}})) {
  test(`startup rejects ${name}`, () => {
    assert.equal(step(handshake(), {kind: 'Input', input: {...packet, ...change}}).classification, 'rejected');
  });
}
for (const boundary of ['Case', 'Configure', 'Lobby', 'Running']) {
  test(`startup window closes permanently at first ${boundary}`, () => {
    let state = step(handshake(), {kind: boundary, phase: 'Running'}).state;
    state = step(state, {kind: 'Lobby', phase: 'Lobby'}).state;
    assert.equal(step(state, {kind: 'Input', input: packet, case_id: null}).classification, 'rejected');
  });
}
test('startup requires observed Hello then complete Welcome identity', () => {
  const invalid = [undefined, step(undefined, {kind: 'Hello'}).state,
    step(undefined, {kind: 'Welcome', identity}).state,
    step(step(undefined, {kind: 'Welcome', identity}).state, {kind: 'Hello'}).state];
  for (const key of Object.keys(identity)) {
    const missing = {...identity}; delete missing[key];
    invalid.push(step(step(undefined, {kind: 'Hello'}).state, {kind: 'Welcome', identity: missing}).state);
  }
  for (const state of invalid) assert.equal(step(state, {kind: 'Input', input: packet}).classification, 'rejected');
});
test('second startup and retry after rejected Input cannot become startup proof', () => {
  for (const first of [packet, {...packet, fire: true}]) {
    const state = step(handshake(), {kind: 'Input', input: first}).state;
    assert.equal(step(state, {kind: 'Input', input: packet}).classification, 'rejected');
  }
});
test('duplicate Hello or Welcome is not a startup handshake', () => {
  for (const event of [{kind: 'Hello'}, {kind: 'Welcome', identity}]) {
    const state = step(handshake(), event).state;
    assert.equal(step(state, {kind: 'Input', input: packet}).classification, 'rejected');
  }
});

function callbackHarness() {
  const source = fs.readFileSync(new URL('./online-hud-smoke.mjs', import.meta.url), 'utf8');
  const report = {protocol: [], lobbies: [], errors: [], startup: {inputs: [], window: null}};
  const context = vm.createContext({assert, URL, startupInputPolicy: api.startupInputPolicy, report,
    address: 'ws://127.0.0.1:1234', elapsed: () => 8257, redact: () => {}, secrets: new Set(),
    identity: undefined, lobby: undefined, activeCase: undefined, inputs: [], commands: [], leaveSent: false,
    fault: (source, message) => report.errors.push({source, message})});
  const helper = source.match(/function observeStartup\(event\) \{[\s\S]*?\n\}/)?.[0] ?? '';
  vm.runInContext(helper + '\n' + source.slice(source.indexOf('function observeSocket(ws)'), source.indexOf('async function installObserver()')), context);
  const socket = new EventEmitter(); socket.url = () => 'ws://127.0.0.1:1234'; context.observeSocket(socket);
  const send = message => socket.emit('framesent', {payload: JSON.stringify(message)});
  const receive = message => socket.emit('framereceived', {payload: JSON.stringify(message)});
  return {context, report, send, receive};
}
test('actual main socket callback retains startup packet separately without errors or case counters', () => {
  const {context, report, send, receive} = callbackHarness();
  send({Hello: {name: 'HudLocal'}}); receive({Welcome: identity}); send({Input: packet});
  assert.deepEqual(report.errors, []);
  assert.equal(context.inputs.length, 1);
  assert.equal(context.inputs[0].case_id, null);
  assert.equal(context.inputs[0].classification, 'startup-neutral');
  assert.equal(report.startup.inputs.length, 1);
  assert.equal(report.startup.window.startup_count, 1);
  send({Input: packet});
  assert.equal(context.inputs.length, 2, 'rejected packet also retained');
  assert.equal(report.errors.length, 1);
  assert.equal(report.startup.inputs.length, 1);
});
test('main callback rejects and retains illegal unscoped Input throughout handshake boundaries', () => {
  for (const scenario of ['preHello', 'preWelcome', 'missingIdentity', 'fire', 'axis', 'sequence', 'Lobby', 'Running', 'Configure']) {
    const {context, report, send, receive} = callbackHarness();
    if (scenario !== 'preHello') send({Hello: {name: 'HudLocal'}});
    if (!['preHello', 'preWelcome'].includes(scenario)) {
      receive({Welcome: scenario === 'missingIdentity' ? {...identity, seat: undefined} : identity});
    }
    if (['Lobby', 'Running'].includes(scenario)) receive({Lobby: {phase: scenario, humans: [identity]}});
    if (scenario === 'Configure') send({Configure: {bots: 1, wins_target: 7}});
    const before = report.errors.length;
    send({Input: {...packet, ...(scenario === 'fire' ? {fire: true} : scenario === 'axis' ? {vertical: 1} : scenario === 'sequence' ? {sequence: 2} : {})}});
    assert.equal(report.errors.length, before + 1, scenario);
    assert.equal(context.inputs.length, 1, scenario);
    assert.equal(context.inputs[0].classification, 'rejected', scenario);
    assert.equal(report.startup.inputs.length, 0, scenario);
  }
});

test('main case input remains strict and unknown commands still fault', () => {
  const {context, report, send} = callbackHarness();
  context.activeCase = {plan: {id: 'preflight'}, stats: {input: {count: 0, neutral_fire: 0}}};
  send({Input: {...packet, sequence: 8, fire: true}});
  assert.equal(context.activeCase.stats.input.count, 1);
  assert.equal(context.activeCase.stats.input.neutral_fire, 1);
  send({Input: {...packet, horizontal: 1}}); send({Restart: true});
  assert.equal(report.errors.length, 2);
  assert.equal(context.inputs.length, 2);
  assert.equal(report.startup.inputs.length, 0);
});
