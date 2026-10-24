// One-shot sanitized supplement; retains the earlier frozen package unchanged.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {gzipSync} from 'node:zlib';
import {execFileSync,spawnSync} from 'node:child_process';
const base='verification/t_84b7da26',root=base+'/final-package',old=base+'/package';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const git=(...args)=>execFileSync('git',args,{maxBuffer:64*1024*1024});
assert(!fs.existsSync(root),'immutable output exists');fs.mkdirSync(root,{recursive:true});
const members=[],receipts=[];
function scan(b){const s=b.toString('utf8');assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(s));assert(!/\b(?:ghp_|gho_|github_pat_)[A-Za-z0-9_]{20,}/.test(s));}
function save(p,b,extra={}){scan(b);const target=root+'/'+p;fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,b,{flag:'wx'});members.push({path:p,sha256:sha(b),bytes:b.length,...extra});}
function copy(from,to){const b=fs.readFileSync(from);save(to,b);receipts.push({original_path:path.resolve(from),published_path:to,original_sha256:sha(b),transformation:'byte-for-byte'});}
function sanitized(from,to){const b=fs.readFileSync(from),obj=JSON.parse(b),codes=new Set();
function collect(v){if(!v||typeof v!=='object')return;for(const [k,x] of Object.entries(v)){if(k==='code'&&typeof x==='string'&&x)codes.add(x);if(/^(?:token|credential|password|authorization|cookie)$/i.test(k))assert(x==='[REDACTED]'||x===null,'unexpected unsanitized field');collect(x);}}collect(obj);
if(obj.failure)obj.failure=String(obj.failure).split('\n')[0];
let text=JSON.stringify(obj);for(const c of codes)text=text.split(c).join('[invitation omitted]');text=text.replace(/\b(?:wss?|https?):\/\/[^\s"'<>\\]+/g,'[local endpoint omitted]');
const decoded=Buffer.from(text+'\n');scan(decoded);save(to,gzipSync(decoded,{level:9}),{encoding:'gzip',decoded_sha256:sha(decoded),decoded_bytes:decoded.length});receipts.push({original_path:path.resolve(from),published_path:to,original_sha256:sha(b),original_bytes:b.length,transformation:'JSON reserialization, invitation/endpoint removal; failure stack omitted after first line; statuses and wire geometry retained. gzip.'});}
const prior='verification/rear-browser/2026-10-24T16-15-17-107Z-163de33ab7b6';
const next=process.argv[2];assert(next&&fs.existsSync(next+'/results.json'),'finished current attempt required');
for(const [name,dir]of[['attempt118',prior],['attempt120',next]]){
for(const p of ['results.json','states.json','inputs.json','lobbies.json','sha256.json']){
if(fs.existsSync(dir+'/'+p))sanitized(dir+'/'+p,name+'/'+p+'.gz');
else receipts.push({original_path:path.resolve(dir+'/'+p),published:false,reason:'Not emitted by failed launcher; hash inputs remain embedded in results.json. Not fabricated.'});
}
for(const p of ['RearA.png','RearB.png'])if(fs.existsSync(dir+'/'+p))copy(dir+'/'+p,name+'/'+p);
fs.chmodSync(dir,0o700);
}
for(const [from,to]of[
[base+'/rear-browser.mjs','provenance/rear-browser118.mjs'],[base+'/rear-replay-provenance/executed.mjs','provenance/executed118.mjs'],
[base+'/rear-browser120.mjs','provenance/rear-browser120.mjs'],[base+'/rear120-provenance/executed.mjs','provenance/executed120.mjs'],[base+'/rear120-provenance/receipt.json','provenance/receipt120.json'],
[base+'/rear-rail-analysis120.mjs','rear-rail-analysis120.mjs'],[base+'/rear-rail-analysis118.json','rear-rail-analysis118.json'],[base+'/rear-rail-analysis120.json','rear-rail-analysis120.json'],
[base+'/final-verify-template.mjs','verify.mjs'],[base+'/assemble-final120.mjs','assemble-final120.mjs'],[base+'/FINAL-README.txt','README.txt'],
[base+'/index-audit-receipt.json','prior-index-audit-receipt.json'],[base+'/tamper-test-results.json','prior-tamper-test-results.json'],
[base+'/cached-check120.mjs','cached-check120.mjs'],[base+'/cached-check120.json','cached-check120.json'],
[base+'/audit121.mjs','audit121.mjs'],[base+'/audit121.json','audit121.json']])copy(from,to);
const rawlogs=[base+'/rear-browser-attempt1.log',base+'/rear-browser120.log'];
for(const p of rawlogs)if(fs.existsSync(p))receipts.push({original_path:path.resolve(p),original_sha256:sha(fs.readFileSync(p)),published:false,reason:'Raw launcher logs withheld; result and provenance derivatives published instead.'});
const original=JSON.parse(fs.readFileSync(old+'/history/originals-reverified.json'));
for(const e of original.preserved)assert.equal(sha(fs.readFileSync(e.path)),e.sha256,'preserved original');assert.equal(sha(fs.readFileSync(original.HUD.path)),original.HUD.sha256,'historical HUD');
save('original-receipts.json',Buffer.from(JSON.stringify({receipts,historical_originals_reverified:original},null,2)+'\n'));
const manifest={schema:1,tested_source_commit:'f50904f6b8cadcd11f6ba86f4a7f26c49b672621',original_manifest_sha256:sha(fs.readFileSync(old+'/MANIFEST.json')),original_reconciliation_sha256:sha(fs.readFileSync(old+'/RECONCILIATION.json')),members:members.sort((a,b)=>a.path.localeCompare(b.path)),self_exclusions:['MANIFEST.json','INDEX-AUDIT.json'],self_exclusion_reason:'No recursively self-referential hashes. Final metadata hashes bound by outside final-head receipt and Kanban handoff.',exclusions:['Private/raw prior failure stacks and raw launcher logs: original paths/hashes only','Original browser JSON retained mode0700: sanitized compressed derivatives only','All unreviewed ignored verification trees and TLS material: not opened, added, or certified','No artifact rebuild; existing external compiler/native bundle paths and hashes remain in original package'],historical_package_status:'Original package/README/MANIFEST/RECONCILIATION are immutable snapshots of run117, including then-pending commit/contact gates; this supplement reports later outcomes without rewriting those historical claims.'};
fs.writeFileSync(root+'/MANIFEST.json',JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
assert.equal(git('diff','--cached','--name-only').toString().trim(),'','refuse preexisting staging');
const paths=members.map(e=>root+'/'+e.path).concat(root+'/MANIFEST.json');
const before=paths.map(p=>{const r=spawnSync('git',['show',':'+p],{maxBuffer:64*1024*1024});return{path:p,disk_sha256:sha(fs.readFileSync(p)),index_sha256:r.status===0?sha(r.stdout):null,ignored:spawnSync('git',['check-ignore','-q','--no-index','--',p]).status===0};});
for(const p of paths)git('add','-f','--',p); // Exact reviewed manifest path, never a directory.
const om=JSON.parse(fs.readFileSync(old+'/MANIFEST.json'));
const all=[...paths,...om.members.map(e=>old+'/'+e.path),...om.self_exclusions.map(p=>old+'/'+p)];
const after=all.map(p=>{const disk=sha(fs.readFileSync(p)),index=sha(git('show',':'+p));assert.equal(index,disk,p);return{path:p,disk_sha256:disk,index_sha256:index,equal:true};});
const sources=JSON.parse(fs.readFileSync(old+'/source-map.json'));
for(const e of sources){assert.equal(sha(fs.readFileSync(e.path)),e.sha256,e.path);assert.equal(sha(git('show',':'+e.path)),e.sha256,e.path);assert.equal(sha(git('show',manifest.tested_source_commit+':'+e.path)),e.sha256,e.path);}
const ignored=git('ls-files','--others','--ignored','--exclude-standard','verification').toString().split('\n').filter(Boolean);
const audit={status:'passed',source_commit:manifest.tested_source_commit,head_before_evidence_only_commit:git('rev-parse','HEAD').toString().trim(),manifest_sha256:sha(fs.readFileSync(root+'/MANIFEST.json')),before,after,source_paths_verified:sources.length,disk_index_differences:[],source_commit_differences:[],ignored_remaining_count:ignored.length,ignored_remaining_policy:'Unreviewed files remain ignored, contents not inspected/certified; no blanket force-add.',self_exclusion:'INDEX-AUDIT.json cannot include its own hash; final verifier checks its exact disk/index bytes, outside final-head receipt binds SHA256.',force_add_policy:'Every path individually allowlisted/scanned; PNGs manually reviewed; one explicit path per add, including non-ignored paths for consistency. No directory or ignored log glob added.'};
fs.writeFileSync(root+'/INDEX-AUDIT.json',JSON.stringify(audit,null,2)+'\n',{flag:'wx'});git('add','-f','--',root+'/INDEX-AUDIT.json');
assert.equal(sha(git('show',':'+root+'/INDEX-AUDIT.json')),sha(fs.readFileSync(root+'/INDEX-AUDIT.json')));
console.log(JSON.stringify({status:'assembled-and-staged',members:members.length,source_members:sources.length,package_paths:all.length+1,manifest_sha256:sha(fs.readFileSync(root+'/MANIFEST.json')),audit_sha256:sha(fs.readFileSync(root+'/INDEX-AUDIT.json')),tree:git('write-tree').toString().trim()}));
