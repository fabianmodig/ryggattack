import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
import {publishSoloEvidence, capturePrivateScreenshot, PrivacyViolation} from './solo-bots-helpers.mjs';
import {redact} from './live-deployment-helpers.mjs';

const repo = fileURLToPath(new URL('../', import.meta.url));
const filesBelow = dir => fs.readdirSync(dir, {withFileTypes: true}).sort((a, b) => a.name.localeCompare(b.name)).flatMap(entry => {
  assert(!entry.isSymbolicLink(), 'source symlinks forbidden');
  const file = path.join(dir, entry.name);
  return entry.isDirectory() ? filesBelow(file) : entry.isFile() ? [file] : [];
});
const withOutput = fn => {
  const scratch = process.env.TMPDIR ?? path.join(os.homedir(), '.hermes/cache/scratch');
  fs.mkdirSync(scratch, {recursive: true});
  const out = fs.mkdtempSync(path.join(scratch, 'explosion-privacy-'));
  try { return fn(out); } finally { fs.rmSync(out, {recursive: true, force: true}); }
};
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

// Match the source-inputs selection at the actual harness publication boundary.
// No browser run or synthetic gameplay claim: only real repository source bytes.
test('explosion full source snapshot survives unchanged privacy publication gates', () => withOutput(out => {
  const files = [...filesBelow(path.join(repo, 'src')).filter(f => f.endsWith('.rs')),
    ...filesBelow(path.join(repo, 'web')),
    ...filesBelow(path.join(repo, 'scripts')).filter(f => /\.(?:mjs|js|sh)$/.test(f)),
    ...['Cargo.toml', 'Cargo.lock', 'docs/online-explosion.md'].map(f => path.join(repo, f))];
  const hashes = {};
  for (const file of files) {
    const relative = path.relative(repo, file), bytes = fs.readFileSync(file);
    hashes[relative] = {sha256: hash(bytes), bytes: bytes.length};
    const target = path.join(out, 'source-inputs', relative);
    fs.mkdirSync(path.dirname(target), {recursive: true});
    fs.writeFileSync(target, bytes);
  }
  const secrets = new Set([['synthetic', 'admission', 'credential'].join('-')]);
  const report = redact({status: 'publication-fixture-only', hashes}, secrets);
  assert.equal(publishSoloEvidence(out, report, secrets), true, 'all copied source bytes must satisfy the same scan as results and screenshots');
  const manifest = JSON.parse(fs.readFileSync(path.join(out, 'sha256.json'), 'utf8'));
  for (const [file, expected] of Object.entries(hashes)) {
    assert.equal(manifest.find(entry => entry.file === 'source-inputs/' + file)?.sha256, expected.sha256);
    assert.equal(hash(fs.readFileSync(path.join(out, 'source-inputs', file))), expected.sha256);
  }
  assert.equal(JSON.parse(fs.readFileSync(path.join(out, 'results.json'), 'utf8')).private_material_negative_scan.all_artifacts_exact_value_scan, true);
}));

// Construct negative fixtures at execution time: their source is publishable,
// but emitted marker bytes must STILL be rejected, even in a copied source file.
test('generated private-key headers still purge all evidence in every publication path', () => {
  for (const prefix of ['', 'RSA ', 'EC ']) for (const location of ['source', 'metadata', 'binary', 'filename']) withOutput(out => {
    const marker = ['-----BEGIN ', prefix, 'PRIVATE KEY', '-----'].join('');
    fs.writeFileSync(path.join(out, 'earlier.png'), Buffer.from([137, 80, 78, 71]));
    let evidence = {status: 'fixture-only'};
    if (location === 'source') {
      fs.mkdirSync(path.join(out, 'source-inputs/scripts'), {recursive: true});
      fs.writeFileSync(path.join(out, 'source-inputs/scripts/fixture.mjs'), JSON.stringify(marker));
    }
    if (location === 'metadata') evidence.note = marker;
    if (location === 'binary') fs.writeFileSync(path.join(out, 'extra.bin'), Buffer.concat([Buffer.from([0, 255]), Buffer.from(marker)]));
    if (location === 'filename') fs.writeFileSync(path.join(out, marker), 'fixture');
    assert.equal(publishSoloEvidence(out, evidence, new Set()), false);
    assert.deepEqual(fs.readdirSync(out), ['results.json']);
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(out, 'results.json'), 'utf8')), {status: 'failed', failure: 'private-material scan failed; artifacts withheld'});
  });
});

test('generated private-key headers still block screenshot pixels before capture', async () => {
  for (const prefix of ['', 'RSA ', 'EC ']) {
    const marker = ['-----BEGIN ', prefix, 'PRIVATE KEY', '-----'].join('');
    let captures = 0;
    const page = {locator: () => ({innerText: async () => marker})};
    await assert.rejects(capturePrivateScreenshot(page, new Set(), async () => { captures++; }), PrivacyViolation);
    assert.equal(captures, 0);
  }
});
