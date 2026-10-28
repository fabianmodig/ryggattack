// Negative tests operate only on owned temporary copies, never retained proof.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {gzipSync,gunzipSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
assert(process.env.TMPDIR,'owned scratch TMPDIR required');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const results=[];
function trial(name,edit,expected){
 const d=fs.mkdtempSync(path.join(process.env.TMPDIR,'explosion-verifier-'));
 try{
  fs.cpSync(root,d,{recursive:true});
  const resign=n=>{const m=JSON.parse(fs.readFileSync(path.join(d,'manifest.json'))),b=fs.readFileSync(path.join(d,n));m[n]={sha256:sha(b),bytes:b.length};fs.writeFileSync(path.join(d,'manifest.json'),JSON.stringify(m));};
  edit?.(d,resign);
  const r=spawnSync(process.execPath,[path.join(d,'verify.mjs')],{encoding:'utf8',timeout:30000,maxBuffer:1024*1024});
  assert(!r.error,name+': child execution');
  if(expected){assert.notEqual(r.status,0,name);assert.match(r.stderr,expected,name);}
  else assert.equal(r.status,0,r.stderr);
  results.push({name,exit_code:r.status,expected_rejection:!!expected});
 }finally{fs.rmSync(d,{recursive:true,force:true});}
}
trial('unaltered self-contained package',null,null);
trial('changed image bytes',d=>fs.appendFileSync(path.join(d,'after-q0/BlastA-1.png'),'changed'),/AssertionError/);
trial('unexpected package member',d=>fs.writeFileSync(path.join(d,'unexpected.txt'),'fixture'),/no missing or unexpected/);
trial('semantically different shared frame despite resigned manifest',(d,resign)=>{
 const n='after-q0/states.json.gz',p=path.join(d,n),s=JSON.parse(gunzipSync(fs.readFileSync(p)));
 const ticks=new Set(s[0].frames.map(f=>f.frame.tick));
 const shared=s[1].frames.find(f=>ticks.has(f.frame.tick));assert(shared);
 shared.frame.carts[0].position[0]+=1;
 fs.writeFileSync(p,gzipSync(JSON.stringify(s)));resign(n);
},/observer frame mismatch/);
trial('altered report checkpoint despite resigned manifest',(d,resign)=>{
 const n='after-q0/results.json',p=path.join(d,n),r=JSON.parse(fs.readFileSync(p));r.matches[0].wall.primary_id+=1;fs.writeFileSync(p,JSON.stringify(r));resign(n);
},/recomputed report checkpoint evidence/);
console.log(JSON.stringify({passed:results.length,results}));
