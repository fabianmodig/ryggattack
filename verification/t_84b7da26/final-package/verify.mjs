// Read-only exact-head/package/index verifier. No build, browser or network.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {gunzipSync} from 'node:zlib';
import {execFileSync} from 'node:child_process';
const repo=execFileSync('git',['rev-parse','--show-toplevel'],{encoding:'utf8'}).trim();
process.chdir(repo);
const rootFlag=process.argv.indexOf('--root');
const root=rootFlag<0?'verification/t_84b7da26/final-package':process.argv[rootFlag+1];
assert(root&&root.startsWith('verification/')&&!root.includes('..'),'repository verification root required');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const git=(...a)=>execFileSync('git',a,{maxBuffer:64*1024*1024});
const walk=d=>fs.readdirSync(d,{withFileTypes:true}).flatMap(e=>{assert(!e.isSymbolicLink());const p=path.join(d,e.name);return e.isDirectory()?walk(p):[p];});
const m=JSON.parse(fs.readFileSync(root+'/MANIFEST.json'));
assert.deepEqual(walk(root).sort(),m.members.map(e=>root+'/'+e.path).concat(m.self_exclusions.map(p=>root+'/'+p)).sort());
function scan(b){const t=b.toString('utf8');assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(t),'private key');assert(!/\b(?:ghp_|gho_|github_pat_)[A-Za-z0-9_]{20,}/.test(t),'credential');}
for(const e of m.members){const b=fs.readFileSync(root+'/'+e.path);assert.equal(sha(b),e.sha256,e.path);assert.equal(b.length,e.bytes,e.path);scan(b);if(e.encoding==='gzip'){const d=gunzipSync(b);assert.equal(sha(d),e.decoded_sha256,e.path);scan(d);}}
for(const [name,count] of [['118',0],['120',1]]){
const r=JSON.parse(gunzipSync(fs.readFileSync(root+'/attempt'+name+'/results.json.gz')));
assert.equal(r.status,'failed','historical launcher failure must remain failed');
assert.equal(r.errors.length,0);assert.equal(r.cleanup.inputs_unchanged,true);
const analysis=JSON.parse(execFileSync('node',[root+'/rear-rail-analysis120.mjs',root+'/attempt'+name],{maxBuffer:16*1024*1024}));
const saved=JSON.parse(fs.readFileSync(root+'/rear-rail-analysis'+name+'.json'));
assert.equal(analysis.event_count,count);assert.deepEqual(analysis.events,saved.events);
assert.equal(analysis.shared_adjacent_pairs,saved.shared_adjacent_pairs);
}
const checks=JSON.parse(fs.readFileSync(root+'/cached-check120.json'));
assert.equal(checks.native_passed,19);assert.equal(checks.node.passed,143);
const old='verification/t_84b7da26/package',before=JSON.parse(fs.readFileSync(old+'/MANIFEST.json'));
assert.equal(sha(fs.readFileSync(old+'/MANIFEST.json')),m.original_manifest_sha256);
assert.equal(sha(fs.readFileSync(old+'/RECONCILIATION.json')),m.original_reconciliation_sha256);
for(const e of before.members)assert.equal(sha(fs.readFileSync(old+'/'+e.path)),e.sha256,e.path);
const sources=JSON.parse(fs.readFileSync(old+'/source-map.json'));
for(const e of sources){assert.equal(sha(fs.readFileSync(e.path)),e.sha256,e.path);assert.equal(sha(git('show',m.tested_source_commit+':'+e.path)),e.sha256,e.path);}
assert.equal(git('diff',m.tested_source_commit,'HEAD','--','src','web','scripts','assets','Cargo.toml','Cargo.lock','.forgejo','docs').length,0,'runtime/test/CI/doc change after acceptance');
const paths=[...before.members.map(e=>old+'/'+e.path),...before.self_exclusions.map(p=>old+'/'+p),...walk(root)];
if(process.argv.includes('--index'))for(const p of [...paths,...sources.map(e=>e.path)])assert.equal(sha(git('show',':'+p)),sha(fs.readFileSync(p)),p+' disk/index');
const h=process.argv.indexOf('--head');if(h>=0)assert.equal(git('rev-parse','HEAD').toString().trim(),process.argv[h+1]);
console.log(JSON.stringify({status:'passed',head:git('rev-parse','HEAD').toString().trim(),tested_source_commit:m.tested_source_commit,original_members:before.members.length,supplement_members:m.members.length,metadata_exclusions:m.self_exclusions,index_checked:process.argv.includes('--index'),source_members:sources.length,package_paths:paths.length,claim:'Hash/source/index identity; inspect contact results and pixels separately. No CI/release/delivery approval.'}));
