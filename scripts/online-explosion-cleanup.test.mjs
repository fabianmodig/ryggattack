import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import fs from 'node:fs';
import {watchOwnedProcess, stopOwnedProcess, stopOwnedBrowser, leaveAdmittedClient, finalizeCleanupReport, watchCleanupInterruption} from './online-explosion-cleanup.mjs';
test('smoke shutdown and finalization invoke the tested shared helpers', () => {
  const source = fs.readFileSync(new URL('./online-explosion-smoke.mjs', import.meta.url), 'utf8');
  assert.match(source, /import \{[^}]*watchOwnedProcess[^}]*stopOwnedProcess[^}]*leaveAdmittedClient[^}]*finalizeCleanupReport[^}]*watchCleanupInterruption[^}]*\} from '.\/online-explosion-cleanup.mjs'/);
  assert.match(source, /backendLifecycle = watchOwnedProcess\(backend\)/);
  assert.match(source, /lifecycle: watchOwnedProcess\(owned.process\(\)\)/);
  assert.match(source, /return leaveAdmittedClient\(c, \{clients, expand, until, presentation\}\)/);
  assert.match(source, /await stopOwnedBrowser\(entry\)/);
  assert.match(source, /await stopOwnedProcess\(backendLifecycle,/);
  assert.equal([...source.matchAll(/if \(!result.valid\) fault\('cleanup',/g)].length, 2);
  assert.match(source, /const cleanupExitCode = finalizeCleanupReport\(report, \{clients, interrupted: interruption.interrupted\}\)/);
  assert.match(source, /process.exitCode \|\|= cleanupExitCode/);
  assert.match(source, /const interruption = watchCleanupInterruption\(process\)/);
});
const never = () => new Promise(() => {});
function mockChild() {
  const child = new EventEmitter();
  child.exitCode = null; child.signalCode = null; child.kills = [];
  child.finish = (code, signal = null) => { child.exitCode = code; child.signalCode = signal; child.emit('exit', code, signal); };
  child.kill = signal => { child.kills.push(signal); if (signal === 'SIGKILL') child.finish(null, signal); return true; };
  return child;
}
for (const owner of ['browser', 'backend']) {
  test(`${owner}: requested clean code-zero exit is accepted`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child);
    const result = await stopOwnedProcess(state, () => child.finish(0), {wait: never});
    assert.equal(result.valid, true); assert.deepEqual(result.exit, {code: 0, signal: null});
    assert.deepEqual(child.kills, []);
  });
  test(`${owner}: already-dead code-zero child is rejected before harness close`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child); child.finish(0);
    let closes = 0;
    const result = await stopOwnedProcess(state, () => { closes++; }, {wait: never});
    assert.equal(closes, 0); assert.equal(result.valid, false);
  });
  test(`${owner}: requested code-one exit is rejected`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child);
    const result = await stopOwnedProcess(state, () => child.finish(1), {wait: never});
    assert.equal(result.valid, false);
  });
  test(`${owner}: externally SIGKILLed exit during requested close is rejected`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child);
    const result = await stopOwnedProcess(state, () => child.finish(null, 'SIGKILL'), {wait: never});
    assert.equal(result.valid, false);
  });
  test(`${owner}: unexpected clean exit queued before request is rejected`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child);
    queueMicrotask(() => child.finish(0));
    const result = await stopOwnedProcess(state, () => {}, {wait: never});
    assert.equal(result.valid, false);
  });
  test(`${owner}: forced close timeout is rejected and only owned child is killed`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child);
    const result = await stopOwnedProcess(state, never, {wait: async () => {}});
    assert.equal(result.valid, false); assert.equal(result.forced, true);
    assert.deepEqual(child.kills, ['SIGKILL']);
  });
}
const admitted = (name = 'A') => ({name, identity: {participant: name}, cleanup: {}});
const successfulCleanup = () => ({leave_packet: true, left_room_readback: true, socket_closed: true});
const freshReport = () => ({status: 'captured-review-required', errors: []});
const leaveDeps = clients => ({clients, expand: async () => {}, presentation: async () => ({phase: null, seat: null, tick: null, names: []}),
  until: async (predicate, label) => assert(predicate(), label)});
test('admitted page already closed cannot silently skip normal Leave', async () => {
  const c = admitted(); c.page = {isClosed: () => true};
  await assert.rejects(leaveAdmittedClient(c, leaveDeps([c])), /admitted.*page/i);
});
test('admitted client with missing page cannot silently skip normal Leave', async () => {
  const c = admitted();
  await assert.rejects(leaveAdmittedClient(c, leaveDeps([c])), /admitted.*page/i);
});
for (const missing of ['leave_packet', 'left_room_readback', 'socket_closed']) {
  test(`finalization rejects closed admitted page missing ${missing}`, () => {
    const c = admitted(); c.cleanup = successfulCleanup(); delete c.cleanup[missing]; c.page = {isClosed: () => true};
    const report = freshReport();
    assert.equal(finalizeCleanupReport(report, {clients: [c], interrupted: false}), 1);
    assert.equal(report.status, 'failed'); assert(report.errors.some(e => e.message.includes(missing)));
  });
}
test('finalization rejects missing first-peer authoritative roster readback', () => {
  const a = admitted('A'), b = admitted('B'); a.cleanup = successfulCleanup(); b.cleanup = successfulCleanup();
  const report = freshReport();
  assert.equal(finalizeCleanupReport(report, {clients: [a, b], interrupted: false}), 1);
  assert(report.errors.some(e => e.message.includes('peer_roster_readback')));
});
test('finalization rejects first-peer readback still containing departing participant', () => {
  const a = admitted('A'), b = admitted('B'); a.cleanup = {...successfulCleanup(), peer_roster_readback: {humans: [{id: 'A'}]}}; b.cleanup = successfulCleanup();
  assert.equal(finalizeCleanupReport(freshReport(), {clients: [a, b], interrupted: false}), 1);
});
test('real Leave helper uses button, waits for cleared HUD, socket and peer roster', async () => {
  // Mocked UI boundary only; never represented as actual gameplay/render proof.
  const a = admitted('A'), b = admitted('B'); b.lobby = {humans: [{id: 'B'}]};
  const actions = [];
  a.page = {isClosed: () => false, keyboard: {up: async key => actions.push(key)},
    locator: selector => ({click: async () => { actions.push(selector); a.leaveSent = true; a.socketClosed = true; }}),
    waitForFunction: async predicate => { assert.match(String(predicate), /hud.seat === null/); actions.push('cleared-HUD'); }};
  await leaveAdmittedClient(a, leaveDeps([a, b])); b.cleanup = successfulCleanup();
  assert.deepEqual(actions, ['Space', '#leave-room:visible, #leave-match:visible', 'cleared-HUD']);
  assert.deepEqual(a.cleanup.peer_roster_readback, b.lobby);
  assert.equal(finalizeCleanupReport(freshReport(), {clients: [a, b], interrupted: false}), 0);
});
test('uncleared HUD rejects Leave and finalization even after page closes', async () => {
  const c = admitted();
  c.page = {isClosed: () => false, keyboard: {up: async () => {}}, locator: () => ({click: async () => { c.leaveSent = true; }}),
    waitForFunction: async () => { throw Error('HUD not cleared'); }};
  await assert.rejects(leaveAdmittedClient(c, leaveDeps([c])), /HUD not cleared/);
  c.page = {isClosed: () => true};
  assert.equal(finalizeCleanupReport(freshReport(), {clients: [c], interrupted: false}), 1);
});
test('nonadmitted startup failures do not invent Leave requirements', async () => {
  const c = {name: 'not admitted', cleanup: {}};
  await leaveAdmittedClient(c, leaveDeps([c]));
  assert.equal(finalizeCleanupReport(freshReport(), {clients: [c], interrupted: false}), 0);
});
for (const signal of ['SIGINT', 'SIGTERM']) {
  test(`${signal} during cleanup forces failed status and nonzero after successful process close`, async () => {
    const signals = new EventEmitter(), interruption = watchCleanupInterruption(signals);
    const child = mockChild(), state = watchOwnedProcess(child);
    const c = admitted(); c.cleanup = successfulCleanup();
    const result = await stopOwnedProcess(state, async () => { signals.emit(signal); child.finish(0); }, {wait: never});
    assert.equal(result.valid, true, 'cleanup is still allowed to finish normally');
    const report = freshReport();
    assert.equal(finalizeCleanupReport(report, {clients: [c], interrupted: interruption.interrupted}), 1);
    assert.equal(report.status, 'failed'); assert(report.errors.some(e => /interrupt/i.test(e.message)));
    interruption.dispose(); assert.equal(signals.listenerCount(signal), 0);
  });
}
test('child startup error is not accepted as a truthy exit record', async () => {
  const child = mockChild(), state = watchOwnedProcess(child);
  child.emit('error', Error('spawn failed'));
  let closes = 0;
  const result = await stopOwnedProcess(state, () => { closes++; }, {wait: never});
  assert.equal(result.valid, false); assert.equal(closes, 0);
});
for (const [code, signal] of [[1, null], [null, 'SIGKILL']]) {
  test(`already dead abnormal child (${code}, ${signal}) is rejected`, async () => {
    const child = mockChild(), state = watchOwnedProcess(child); child.finish(code, signal);
    assert.equal((await stopOwnedProcess(state, () => assert.fail('must not close dead process'), {wait: never})).valid, false);
  });
}
test('existing failure and errors remain fatal with no interruption', () => {
  const failed = {status: 'failed', errors: []};
  assert.equal(finalizeCleanupReport(failed, {clients: [], interrupted: false}), 1);
  const errors = freshReport(); errors.errors.push({source: 'browser', message: 'failed'});
  assert.equal(finalizeCleanupReport(errors, {clients: [], interrupted: false}), 1);
  assert.equal(errors.status, 'failed');
});
function virtualClock() {
  let now = 0;
  const timers = [];
  const flush = async () => { for (let i = 0; i < 20; i++) await Promise.resolve(); };
  return {
    wait: ms => new Promise(resolve => timers.push({at: now + ms, resolve})),
    async advance(to) {
      await flush();
      while (timers.some(timer => timer.at <= to)) {
        timers.sort((a, b) => a.at - b.at);
        const timer = timers.shift(); now = timer.at; timer.resolve(); await flush();
      }
      now = to; await flush();
    },
  };
}
for (const disconnectStalls of [false, true]) {
  test(`browser: observed 5924ms owned close remains clean (disconnect stalls=${disconnectStalls})`, async () => {
    const child = mockChild(), lifecycle = watchOwnedProcess(child), clock = virtualClock(), calls = [];
    const entry = {lifecycle,
      browser: {close: () => { calls.push('disconnect'); return disconnectStalls ? never() : Promise.resolve(); }},
      owned: {close: async () => { calls.push('owner'); await clock.wait(5924); child.finish(0); }}};
    const pending = stopOwnedBrowser(entry, {wait: clock.wait});
    await clock.advance(5000);
    assert.deepEqual(child.kills, [], 'five seconds must not kill a normally closing SwiftShader browser');
    await clock.advance(disconnectStalls ? 10924 : 5924);
    const result = await pending;
    assert.equal(result.valid, true); assert.equal(result.forced, false);
    assert.deepEqual(result.exit, {code: 0, signal: null});
    assert.deepEqual(calls, ['disconnect', 'owner']);
  });
}
test('browser: hung owner is still forcibly stopped and rejected at bounded 20s deadline', async () => {
  const child = mockChild(), lifecycle = watchOwnedProcess(child), clock = virtualClock();
  const pending = stopOwnedBrowser({lifecycle, browser: {close: never}, owned: {close: never}}, {wait: clock.wait});
  await clock.advance(19999); assert.deepEqual(child.kills, []);
  await clock.advance(20000);
  const result = await pending;
  assert.equal(result.valid, false); assert.equal(result.forced, true);
  assert.deepEqual(child.kills, ['SIGKILL']);
});
for (const [code, signal] of [[1, null], [null, 'SIGKILL']]) {
  test(`browser policy still rejects abnormal requested exit (${code}, ${signal})`, async () => {
    const child = mockChild(), lifecycle = watchOwnedProcess(child);
    const result = await stopOwnedBrowser({lifecycle, owned: {close: async () => child.finish(code, signal)}}, {wait: never});
    assert.equal(result.valid, false); assert.equal(result.requested, true);
  });
}
test('graceful backend SIGTERM request must still produce code zero without signal', async () => {
  const child = mockChild(), state = watchOwnedProcess(child);
  child.kill = signal => { child.kills.push(signal); child.finish(0); return true; };
  const result = await stopOwnedProcess(state, async () => { child.kill('SIGTERM'); await state.exit; }, {wait: never});
  assert.equal(result.valid, true); assert.deepEqual(child.kills, ['SIGTERM']);
});
