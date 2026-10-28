// Shared by the actual local smoke shutdown and its mocked lifecycle tests.
import assert from 'node:assert/strict';
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
export function watchCleanupInterruption(signals) {
  const state = {interrupted: false};
  const interrupt = () => { state.interrupted = true; };
  signals.on('SIGINT', interrupt); signals.on('SIGTERM', interrupt);
  state.dispose = () => { signals.removeListener('SIGINT', interrupt); signals.removeListener('SIGTERM', interrupt); };
  return state;
}
export function watchOwnedProcess(child) {
  const state = {child, record: null, requested: false, unexpected: false};
  state.exit = new Promise(resolve => {
    child.once('exit', (code, signal) => { state.unexpected ||= !state.requested; state.record = {code, signal}; resolve(state.record); });
    child.once('error', () => { state.unexpected = true; state.record = {error: true}; resolve(state.record); });
  });
  return state;
}
export async function stopOwnedProcess(state, requestClose, {wait = sleep, closeMs = 5000, exitMs = 2000} = {}) {
  const {child} = state;
  const dead = () => state.record !== null || child.exitCode !== null || child.signalCode !== null;
  const alreadyExited = dead();
  let forced = false;
  try {
    if (!alreadyExited) {
      const closed = await Promise.race([Promise.resolve().then(() => {
        // Recheck at the request boundary: even code-zero exits before it fail.
        if (dead()) return;
        state.requested = true;
        return requestClose();
      }).then(() => true), wait(closeMs).then(() => false)]);
      if (!closed) { forced = true; child.kill('SIGKILL'); }
    }
  } catch { forced = true; child.kill('SIGKILL'); }
  const exit = alreadyExited ? state.record ?? {code: child.exitCode, signal: child.signalCode} : await Promise.race([state.exit, wait(exitMs).then(() => null)]);
  return {exit, forced, requested: state.requested, unexpected: state.unexpected,
    valid: state.requested && !state.unexpected && !forced && exit?.code === 0 && exit?.signal === null && !exit?.error};
}

export async function stopOwnedBrowser(entry, {wait = sleep} = {}) {
  // Actual two-client SwiftShader teardown took 5924ms after owned.close;
  // disconnect-first and owner-first both exceeded the old 5s watchdog.
  // This is a shutdown-only bound, not an extension of gameplay/capture time.
  return stopOwnedProcess(entry.lifecycle, async () => {
    if (entry.browser) await Promise.race([entry.browser.close(), wait(5000)]);
    await entry.owned.close();
  }, {wait, closeMs: 20000});
}

export async function leaveAdmittedClient(c, {clients, expand, until, presentation}) {
  if (!c.identity) return;
  assert(c.page && !c.page.isClosed(), 'admitted client page unavailable before normal Leave');
  if (c.cleanup.left_room_readback) return;
  await c.page.keyboard.up('Space'); await expand(c); await c.page.locator('#leave-room:visible, #leave-match:visible').click();
  await until(() => c.leaveSent, `${c.name}: normal Leave packet`, 10000, false, true);
  await c.page.waitForFunction(() => {
    const room = JSON.parse(globalThis.ryggattackRoom ?? '{}'), hud = JSON.parse(globalThis.ryggattackHud ?? '{}');
    const root = document.querySelector('.rygg-online-hud');
    return room.status?.includes('Left room') && !room.lobby && !hud.lobby && hud.seat === null && hud.frame === null && root?.querySelectorAll('.oh-player').length === 0 && root.querySelector('.oh-outcome').hidden;
  }, null, {timeout: 10000});
  c.cleanup.leave_packet = true; c.cleanup.left_room_readback = true;
  await until(() => c.socketClosed, `${c.name}: normal socket close`, 10000, false, true);
  c.cleanup.socket_closed = true;
  const peer = clients.find(other => other !== c && other.identity && !other.leaveSent);
  if (peer) {
    await until(() => peer.lobby && !peer.lobby.humans.some(h => h.id === c.identity.participant), 'peer authoritative Leave roster readback', 10000, false, true);
    c.cleanup.peer_roster_readback = peer.lobby;
  }
  c.cleanup.readback = await presentation(c);
}

export function finalizeCleanupReport(report, {clients, interrupted}) {
  if (interrupted) report.errors.push({source: 'shutdown', message: 'interrupted before cleanup finalization'});
  const admitted = clients.filter(c => c.identity);
  const missing = (c, field) => report.errors.push({source: `${c.name}:cleanup`, message: `required ${field} missing or invalid`});
  for (const c of admitted) {
    for (const field of ['leave_packet', 'left_room_readback', 'socket_closed']) {
      if (c.cleanup[field] !== true) missing(c, field);
    }
  }
  // Shutdown attempts clients in admission order, even when an earlier page died.
  if (admitted.length > 1) {
    const first = admitted[0], humans = first.cleanup.peer_roster_readback?.humans;
    if (!Array.isArray(humans) || humans.some(h => h.id === first.identity.participant)) missing(first, 'peer_roster_readback');
  }
  if (report.errors.length) report.status = 'failed';
  return report.status === 'captured-review-required' ? 0 : 1;
}
