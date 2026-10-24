import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {gzipSync,gunzipSync} from 'node:zlib';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const read=p=>fs.readFileSync(path.join(root,p));
const walk=(dir,prefix='')=>fs.readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name)).flatMap(e=>{assert(!e.isSymbolicLink(),'symlink forbidden');return e.isDirectory()?walk(path.join(dir,e.name),prefix+e.name+'/'):[prefix+e.name];});
const m=JSON.parse(read('MANIFEST.json'));
const expected=m.members.map(x=>x.path).concat(m.self_exclusions).sort();
assert.deepEqual(walk(root).sort(),expected,'unexpected or missing member');
const scan=b=>{const text=b.toString('utf8');assert(!/-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(text),'key material');assert(!/\b(?:ghp_|gho_|github_pat_)[A-Za-z0-9_]{20,}/.test(text),'access credential');};
for(const e of m.members){const b=read(e.path);assert.equal(sha(b),e.sha256,e.path);assert.equal(b.length,e.bytes,e.path);scan(b);if(e.encoding==='gzip'){const plain=gunzipSync(b);assert.equal(sha(plain),e.decoded_sha256,e.path);assert.equal(plain.length,e.decoded_bytes,e.path);scan(plain);}}
assert.equal(JSON.parse(gunzipSync(read('browser/explosion/results.json.gz'))).status,'captured-review-required');
assert.equal(JSON.parse(gunzipSync(read('browser/hud/results.json.gz'))).status,'passed');
const failed=JSON.parse(read('history/failed-privacy-attempt-original.json'));assert.equal(failed.status,'failed');
if(process.argv.includes('--checkout')){
 const repo=execFileSync('git',['rev-parse','--show-toplevel'],{cwd:root,encoding:'utf8'}).trim();
 const sources=JSON.parse(read('source-map.json'));
 for(const e of sources)assert.equal(sha(fs.readFileSync(path.join(repo,e.path))),e.sha256,e.path);
 const built=JSON.parse(read('parent/retained-verification.json'));
 for(const e of built.wasm.outputs)assert.equal(sha(fs.readFileSync(path.join(repo,'dist/web',e.path))),e.sha256,e.path);
 for(const e of built.native_artifacts)assert.equal(sha(fs.readFileSync(e.path)),e.sha256,e.path);
}
console.log(JSON.stringify({status:'passed',members:m.members.length,self_exclusions:m.self_exclusions,checkout:process.argv.includes('--checkout'),claim:'Integrity and declared results, not independent visual acceptance or final committed-head approval.'}));
