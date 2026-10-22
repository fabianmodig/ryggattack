import test from 'node:test';
import assert from 'node:assert/strict';
import * as helpers from './live-deployment-helpers.mjs';

test('canonical snapshots sort keys and normalize native f32 promotion, not different states', () => {
  assert.deepEqual(helpers.canonical({z:[{x:0.62}],a:42}), helpers.canonical({a:42,z:[{x:0.6200000047683716}]}));
  assert.equal(JSON.stringify(helpers.canonical({z:1,a:2})), '{"a":2,"z":1}');
  assert.notDeepEqual(helpers.canonical({x:0.62}), helpers.canonical({x:0.63}));
  assert.equal(helpers.canonical(123456789),123456789);
});

test('redaction collects nested credential/token/reclaim values and scrubs free text without mutating input', () => {
  const secrets = new Set();
  const input = {Welcome:{participant:9,seat:0,credential:'private-credential'}, nested:{access_token:'private-token', reclaim:{proof:'private-proof'}}, log:'private-token / private-credential / private-proof'};
  const safe = helpers.redact(input, secrets);
  assert.equal(safe.Welcome.participant,9);
  assert.equal(safe.Welcome.seat,0);
  assert.equal(safe.Welcome.credential,'[REDACTED]');
  assert.equal(safe.nested.reclaim,'[REDACTED]');
  assert.equal(safe.log,'[REDACTED] / [REDACTED] / [REDACTED]');
  assert.equal(input.Welcome.credential,'private-credential');
  for (const secret of ['private-credential','private-token','private-proof']) {
    assert(secrets.has(secret));
    assert(!JSON.stringify(safe).includes(secret));
  }
  assert.equal(helpers.redact('later private-token',secrets),'later [REDACTED]');
});
