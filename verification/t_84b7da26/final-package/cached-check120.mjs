// Cached-binary/Node regression execution. No compiler or build invoked.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const log=fs.readFileSync('verification/t_621f9087-run106/cargo-test.log','utf8');
const bin=/Running unittests src\/lib.rs \(([^)]+)\)/.exec(log)[1];
const before=sha(fs.readFileSync(bin)),runs=[];
for(const filter of ['online::tests::','server::parity_tests::offline_server_entry_physical_rear_contact','players::tests::shared_fixed_tick_rear_end']){
const start=performance.now(),out=execFileSync('./'+bin,[filter,'--nocapture'],{encoding:'utf8',timeout:120000});const m=/test result: ok\. (\d+) passed; 0 failed;/.exec(out);assert(m);runs.push({command:['./'+bin,filter,'--nocapture'],passed:Number(m[1]),duration_ms:performance.now()-start,output:out});}
assert.equal(runs.reduce((n,r)=>n+r.passed,0),19);
const tests=fs.readdirSync('scripts').filter(p=>p.endsWith('.test.mjs')&&(p.startsWith('online-explosion')||p.startsWith('online-hud')||['client-feedback-helpers.test.mjs','solo-bots-helpers.test.mjs'].includes(p))).sort().map(p=>'scripts/'+p);
const start=performance.now(),out=execFileSync('node',['--test',...tests],{encoding:'utf8',timeout:120000});assert(out.includes('# fail 0'));const pass=Number(/# pass (\d+)/.exec(out)[1]);assert.equal(pass,143);
assert.equal(sha(fs.readFileSync(bin)),before);
console.log(JSON.stringify({status:'passed',source_head:execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),native_binary:{path:bin,sha256:before},native_passed:19,runs,node:{command:['node','--test',...tests],passed:pass,duration_ms:performance.now()-start,summary:out.split('\n').filter(l=>/^# (tests|pass|fail|duration_ms)/.test(l))},scope:'Fresh cached native test binary and Node execution, not a Cargo compile or full203 Rust rerun; no browser operation in this script.'},null,2));
