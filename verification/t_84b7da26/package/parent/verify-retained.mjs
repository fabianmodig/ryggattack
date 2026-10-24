import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

// Read only explicit source/build evidence; never enumerate environment or credentials.
const root = process.cwd();
const prior = path.join(root, 'verification/t_621f9087-run106');
const packaging = '/home/hermes/.hermes/artifacts/ryggattack/wasm-packaging-20261024';
const output = path.join(root, 'verification/t_621f9087-run113');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const fileHash = file => hash(fs.readFileSync(file));
const read = name => fs.readFileSync(path.join(prior, name), 'utf8');
const entries = read('source-inputs.sha256').trim().split('\n').map(line => {
  const match = /^([a-f0-9]{64})  (.+)$/.exec(line);
  assert(match, 'invalid manifest line');
  assert(!path.isAbsolute(match[2]) && !match[2].split('/').includes('..'));
  assert.equal(fileHash(path.join(root, match[2])), match[1], `source mismatch: ${match[2]}`);
  return { path: match[2], sha256: match[1] };
});
assert.equal(entries.length, 119);
const build = JSON.parse(fs.readFileSync(path.join(packaging, 'result.json'), 'utf8'));
assert.equal(build.status, 'passed');
assert.equal(build.verified_manifest_entries, entries.length);
assert.equal(fileHash(path.join(root, 'target/wasm32-unknown-unknown/release/ryggattack.wasm')), build.input_wasm_sha256);
const bundle = Object.entries(build.outputs).map(([name, expected]) => {
  const bytes = fs.readFileSync(path.join(packaging, 'bundle', name));
  assert.equal(bytes.length, expected.bytes, name);
  assert.equal(hash(bytes), expected.sha256, name);
  return { path: name, ...expected };
});
assert.equal(bundle.length, 9);
assert(WebAssembly.validate(fs.readFileSync(path.join(packaging, 'bundle/ryggattack_bg.wasm'))));
execFileSync('node', ['--check', path.join(packaging, 'bundle/ryggattack.js')]);
const cargoLog = read('cargo-test.log');
const suites = [...cargoLog.matchAll(/test result: ok\. (\d+) passed; (\d+) failed;/g)].map(m => ({passed: Number(m[1]), failed: Number(m[2])}));
assert(suites.length > 0);
assert(suites.every(s => s.failed === 0));
const requiredTests = [
 'online::tests::replicated_fireball_mesh_matches_source_world_bounds_through_its_lifecycle',
 'online::tests::projectile_and_pickup_replica_geometry_stays_unchanged',
 'server::parity_tests::offline_server_entry_physical_rear_contact_pushes_only_leader_point_four_without_reversals',
 'players::tests::shared_fixed_tick_rear_end_pushes_only_front_without_reversing',
];
for (const name of requiredTests) assert(cargoLog.includes(`test ${name} ... ok`), name);
assert(read('red-retry.log').includes('test result: FAILED. 0 passed; 1 failed;'));
assert(read('node.log').includes('# pass 157\n# fail 0'));
assert(read('headless.log').includes('PASS: no-display movement'));
const timings = {};
const evidence = [];
for (const name of ['red', 'red-retry', 'cargo-test', 'clippy', 'node', 'native-build', 'headless', 'other-authoring', 'wasm-build']) {
  const time = read(`${name}.time`);
  timings[name] = {
    elapsed_seconds: Number(/elapsed_seconds=([\d.]+)/.exec(time)[1]),
    exit_code: Number(/exit_code=(-?\d+)/.exec(time)[1]),
  };
  for (const suffix of ['log', 'time']) evidence.push({path: path.join(prior, `${name}.${suffix}`), sha256: fileHash(path.join(prior, `${name}.${suffix}`))});
}
assert.equal(timings['cargo-test'].exit_code, 0);
assert.equal(timings['red-retry'].exit_code, 101);
for (const name of ['clippy', 'node', 'native-build', 'headless', 'other-authoring']) assert.equal(timings[name].exit_code, 0);
const base = execFileSync('git', ['rev-parse', 'HEAD'], {encoding:'utf8'}).trim();
assert.equal(base, build.source_head);
execFileSync('git', ['diff', '--check']);
const diff = execFileSync('git', ['diff', '--binary']);
assert.equal(hash(diff), fileHash(path.join(prior, 'candidate.patch')), 'tracked candidate patch changed');
const native = ['ryggattack-server', 'ryggattack-headless'].map(name => ({path: path.join(root, 'target/debug', name), sha256: fileHash(path.join(root, 'target/debug', name))}));
const summary = {
  status: 'retained-cpu-and-wasm-evidence-verified-browser-pending',
  command: 'node verification/t_621f9087-run113/verify-retained.mjs',
  base_revision: base,
  candidate_identity: 'Dirty feature worktree, NOT a committed proposed head; use source manifest plus tracked patch digest.',
  source_manifest: path.join(prior, 'source-inputs.sha256'),
  source_manifest_sha256: fileHash(path.join(prior, 'source-inputs.sha256')),
  source_entries_verified: entries.length,
  tracked_candidate_patch_sha256: hash(diff),
  cpu: {passed: suites.reduce((sum, s) => sum + s.passed, 0), failed: 0, suites, requiredTests, timings, clippy_warning_count: 4, node_passed: 157},
  evidence,
  wasm: {packaging_result: path.join(packaging, 'result.json'), packaging_result_sha256: fileHash(path.join(packaging, 'result.json')), stages: build.stages.map(s => ({name:s.name, argv:s.argv, elapsed_seconds:s.elapsed_seconds, exit_code:s.exit_code})), bundle_root: build.bundle, outputs: bundle, current_webassembly_validation: true, current_js_syntax: true},
  native_artifacts: native,
  fresh_builds_run: false,
  limitations: [
    'Prior CPU invocation argv is not stored in run106 logs; do not infer exact flags from output. Reproduction commands are documented separately, not claimed executed in this run.',
    'Initial RED failed to invoke cargo (127); retry reproduced real mesh failure (101). Original WASM wrapper ended 143 after compiler success; standalone packaging supersedes packaging failure, not historical records.',
    'Native artifact hashes measured now; native build log lacks historical artifact digests.',
    'Browser rendering, manual six-image review, HUD/offline/rear-contact browser checks remain downstream. Do not use stale dist/web without checking artifact identity.',
    'Anonymous-bearer theft limitation and deferred native manual graphics unchanged.',
    'No commit, publication, deployment, public-room acceptance, source edit or production restart performed.',
    'Original logs retained privately/in place; this allowlisted summary is a sanitized derivative, not certification that all historical originals are safe to publish.',
  ],
};
fs.writeFileSync(path.join(output, 'retained-verification.json'), JSON.stringify(summary, null, 2) + '\n', {flag:'wx', mode:0o600});
console.log(JSON.stringify({status:summary.status, source_entries:entries.length, cpu_passed:summary.cpu.passed, bundle_files:bundle.length, source_manifest_sha256:summary.source_manifest_sha256, report_sha256:fileHash(path.join(output, 'retained-verification.json'))}, null, 2));
