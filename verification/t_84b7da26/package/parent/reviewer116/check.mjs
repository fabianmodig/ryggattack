import fs from 'node:fs';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
const digest = b => crypto.createHash('sha256').update(b).digest('hex');
const hash = p => digest(fs.readFileSync(p));
const report = JSON.parse(fs.readFileSync('verification/t_621f9087-run113/retained-verification.json'));
for (const x of report.evidence) assert.equal(hash(x.path), x.sha256, x.path);
for (const x of report.native_artifacts) assert.equal(hash(x.path), x.sha256, x.path);
assert.equal(hash(report.source_manifest), report.source_manifest_sha256);
assert.equal(digest(execFileSync('git',['diff','--binary'])),report.tracked_candidate_patch_sha256);
assert.equal(hash(report.wasm.packaging_result),report.wasm.packaging_result_sha256);
for (const x of report.wasm.outputs) {
 const p = report.wasm.bundle_root+'/'+x.path;
 assert.equal(hash(p),x.sha256,p);
 assert.equal(fs.statSync(p).size,x.bytes,p);
}
assert(WebAssembly.validate(fs.readFileSync(report.wasm.bundle_root+'/ryggattack_bg.wasm')));
execFileSync('node',['--check',report.wasm.bundle_root+'/ryggattack.js']);
const log=fs.readFileSync('verification/t_621f9087-run106/cargo-test.log','utf8');
const suites=[...log.matchAll(/test result: ok\. (\d+) passed; (\d+) failed;/g)];
assert.equal(suites.reduce((n,m)=>n+Number(m[1]),0),203);
assert(suites.every(m=>m[2]==='0'));
for (const t of report.cpu.requiredTests) assert(log.includes('test '+t+' ... ok'));
console.log('Independent hashes/log audit passed; 203 retained Rust passes; 9 bundle files; JS syntax and WASM validity passed.');
const bin=/Running unittests src\/lib.rs \(([^)]+)\)/.exec(log)[1];
for (const filter of ['online::tests::','server::parity_tests::offline_server_entry_physical_rear_contact','players::tests::shared_fixed_tick_rear_end']) {
 const out=execFileSync('./'+bin,[filter,'--nocapture'],{encoding:'utf8',timeout:120000});
 assert(/test result: ok\. [1-9]\d* passed; 0 failed;/.test(out));
 console.log('Fresh cached binary execution '+filter+'\n'+out);
}
const tests=fs.readdirSync('scripts').filter(p=>p.endsWith('.test.mjs')&&(p.startsWith('online-explosion')||p.startsWith('online-hud')||['client-feedback-helpers.test.mjs','solo-bots-helpers.test.mjs'].includes(p))).map(p=>'scripts/'+p);
const out=execFileSync('node',['--test',...tests],{encoding:'utf8',timeout:120000});
assert(out.includes('# fail 0'));
console.log(out.split('\n').filter(l=>/^# (tests|pass|fail|duration_ms)/.test(l)).join('\n'));
console.log('No Cargo builds, product edits, browser runs or production operations performed.');
