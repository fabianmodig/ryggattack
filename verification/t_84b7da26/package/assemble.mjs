import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {gzipSync} from 'node:zlib';
const base='verification/t_84b7da26',dest=base+'/package';
assert(!fs.existsSync(dest),'immutable output already exists');
fs.mkdirSync(dest,{recursive:true});
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const receipts=[],members=[];
const scan=(b,name)=>{const t=b.toString('utf8');assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(t),'key material in '+name);assert(!/\b(?:ghp_|gho_|github_pat_)[A-Za-z0-9_]{20,}/.test(t),'access credential in '+name);};
function save(name,b,meta={}){scan(b,name);const f=dest+'/'+name;fs.mkdirSync(path.dirname(f),{recursive:true});fs.writeFileSync(f,b,{flag:'wx'});members.push({path:name,sha256:sha(b),bytes:b.length,...meta});}
function copy(from,to,{zip=false,sanitize=false}={}){
 const original=fs.readFileSync(from);let b=original;
 if(sanitize){
 let text=b.toString('utf8');const parsed=JSON.parse(text),codes=new Set();
 function collect(x){if(!x||typeof x!=='object')return;for(const [k,v] of Object.entries(x)){if(k==='code'&&typeof v==='string'&&v)codes.add(v);collect(v);}}
 collect(parsed);for(const code of codes)text=text.split(code).join('[invitation omitted]');
 text=text.replace(/\b(?:wss?|https?):\/\/[^\s"'<>\\]+/g,'[local endpoint omitted]');b=Buffer.from(text);
 }
 scan(b,to);
 const meta={original_path:path.resolve(from),original_sha256:sha(original),original_bytes:original.length,transformation:sanitize?'Removed invitation values and endpoint URLs; all result statuses preserved.':'byte-for-byte'};
 if(zip){meta.encoding='gzip';meta.decoded_sha256=sha(b);meta.decoded_bytes=b.length;b=gzipSync(b,{level:9});}
 save(to,b,meta);receipts.push({path:to,...meta});
}
const sourceManifest='verification/t_621f9087-run106/source-inputs.sha256';
const sources=fs.readFileSync(sourceManifest,'utf8').trim().split('\n').map(line=>{const [,hash,p]=/^([a-f0-9]{64})  (.+)$/.exec(line);assert.equal(sha(fs.readFileSync(p)),hash,p);return{path:p,sha256:hash};});
for(const p of ['.forgejo/workflows/ci.yml','docs/online-explosion.md'])if(!sources.some(x=>x.path===p))sources.push({path:p,sha256:sha(fs.readFileSync(p))});
for(const e of sources)copy(e.path,'source-inputs/'+e.path);
save('source-map.json',Buffer.from(JSON.stringify(sources,null,2)+'\n'));
copy(sourceManifest,'parent/source-inputs.sha256');
for(const p of ['HANDOFF.txt','retained-verification.json','verify-retained.mjs'])copy('verification/t_621f9087-run113/'+p,'parent/'+p);
save('parent/candidate-patch-receipt.json',Buffer.from(JSON.stringify({original_path:path.resolve('verification/t_621f9087-run106/candidate.patch'),sha256:sha(fs.readFileSync('verification/t_621f9087-run106/candidate.patch')),published:false,reason:'Patch includes removed synthetic negative-test key-header text. Strict scanner rejected it; original retained, raw patch excluded, exact tested final source bytes archived instead.'},null,2)+'\n'));
for(const stem of ['red','red-retry','cargo-test','clippy','native-build','headless','node','other-authoring','wasm-build'])for(const ext of ['log','time']){
 const p='verification/t_621f9087-run106/'+stem+'.'+ext;if(fs.existsSync(p))copy(p,'parent/run106/'+stem+'.'+ext);
}
for(const p of ['check.mjs','check.log'])copy('verification/t_621f9087-review116/'+p,'parent/reviewer116/'+p);
for(const p of ['assemble.mjs','packaging-attempt1-failure.json','preflight.mjs','preflight.json','offline.mjs','rear-wire.mjs','rear-wire.json','cached-cpu-node.log','browser-attempt1.log','hud-attempt1.log','offline-attempt1.log','README.txt'])copy(base+'/'+p,p);
copy(base+'/verify-template.mjs','verify.mjs');
for(const [name,root] of [['explosion','verification/online-explosion/2026-10-24T15-30-37-066Z-fe8307a389d1'],['hud','verification/online-hud/2026-10-24T15-33-35-458Z-901957'],['offline',base+'/offline']]){
 for(const file of fs.readdirSync(root).sort()){
 if(file==='source-inputs')continue;
 assert(/\.(json|png)$/.test(file),'unreviewed browser member');
 const json=file.endsWith('.json');copy(root+'/'+file,'browser/'+name+'/'+file+(json?'.gz':''),{zip:json,sanitize:json});
 }
}
const baseline='/home/hermes/.hermes/artifacts/ryggattack/baseline-t_7969543b';
copy(baseline+'/BASELINE.txt','history/BASELINE.txt');copy(baseline+'/preserved-sha256.txt','history/preserved-sha256.txt');
copy(baseline+'/restricted/failed-privacy-attempt-original.json','history/failed-privacy-attempt-original.json');
const preserved=fs.readFileSync(baseline+'/preserved-sha256.txt','utf8').trim().split('\n').map(line=>{const [,hash,p]=/^([a-f0-9]{64})  (.+)$/.exec(line);assert.equal(sha(fs.readFileSync(p)),hash,'historical original changed');return{path:p,sha256:hash,mode:fs.statSync(p).mode&0o777};});
const hud='/home/hermes/.hermes/artifacts/ryggattack/rollout-2026-10-23/ryggattack-hud-11b9afc-verified.json';
const hudHash=sha(fs.readFileSync(hud));assert.equal(hudHash,'4ddd0c29cdfbccc4857bfa9e9a1ed2876c98b6e1894f9bc9877c5fb1e3183453');
save('history/originals-reverified.json',Buffer.from(JSON.stringify({historical_only:true,preserved,HUD:{path:hud,sha256:hudHash,mode:fs.statSync(hud).mode&0o777},historical_runtime:'11b9afce48aec0d9ff7ea878003c4ca5365e6f6b',historical_chart_main_flux:'f5ae645545babe5e4127d27c8d22ca123839ff66',private_originals_published:false},null,2)+'\n'));
save('producer-receipts.json',Buffer.from(JSON.stringify(receipts,null,2)+'\n'));
const manifest={schema:1,status:'dirty-source-acceptance-package-final-sha-and-browser-rear-contact-pending',members:members.sort((a,b)=>a.path.localeCompare(b.path)),self_exclusions:['MANIFEST.json','RECONCILIATION.json'],exclusion_reason:'Manifest cannot hash itself; reconciliation is derived from final manifest/index and independently read back. Exact metadata hashes must be bound by later final-head receipt.',excluded:['All other ignored verification evidence (not blanket-approved)','Private historical HUD original (hash receipt only)','Private/restricted historical raw records except failed marker (hash receipts only)','TLS keys/certs and auth/environment (never opened or copied)','Build binaries/bundle (durable external paths and hashes only)','Original unsanitized browser JSON (sanitized lossless-gzip derivatives with distinct original/decoded hashes retained)','Earlier historical RED/GREEN raw files remain in restricted baseline, not republished as current-head proof'],scope_boundaries:['No product changes or builds in this task','Browser gates sequential and no unrelated SwiftShader game','Rear0.4 leader-only/no-reversal CPU oracle passes; no qualifying browser-wire event captured','Native manual graphics deferred','Anonymous currently valid bearer theft limitation unchanged','No final head, push, merge, deployment or public-room acceptance']};
fs.writeFileSync(dest+'/MANIFEST.json',JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
fs.writeFileSync(dest+'/RECONCILIATION.json',JSON.stringify({status:'not-yet-audited'},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({status:'assembled',members:members.length,source_files:sources.length,PNGs:members.filter(x=>x.path.endsWith('.png')&&x.path.startsWith('browser/')).length,manifest_sha256:sha(fs.readFileSync(dest+'/MANIFEST.json'))}));
