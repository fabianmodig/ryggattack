// Read-only, offline verifier. Run from any directory; --checkout also checks
// the current source and locally retained compiled artifacts against the replay.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {gunzipSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';
import {assertSharedExplosionEvidence} from './harness/online-explosion-evidence.mjs';
import {canonical} from './harness/live-deployment-helpers.mjs';
const root=path.dirname(fileURLToPath(import.meta.url));
const repo=path.resolve(root,'../../..');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const json=n=>JSON.parse(fs.readFileSync(path.join(root,n)));
function files(d){return fs.readdirSync(d,{withFileTypes:true}).flatMap(e=>{assert(!e.isSymbolicLink(),'no evidence symlinks');const f=path.join(d,e.name);return e.isDirectory()?files(f):[path.relative(root,f)];}).sort();}
const manifest=json('manifest.json');
assert.deepEqual(files(root),[...Object.keys(manifest),'manifest.json'].sort(),'no missing or unexpected package paths');
for(const [n,m] of Object.entries(manifest)){const b=fs.readFileSync(path.join(root,n));assert.equal(b.length,m.bytes,n);assert.equal(sha(b),m.sha256,n);assert(!/\.(pem|key)$/.test(n),'no private key artifacts');}
let compared=0, shared=0, recordedShared=0;
for(const label of ['after-q0','after-q6']){
 const r=json(label+'/results.json');
 assert.equal(r.status,'captured-review-required');assert.equal(r.errors.length,0);
 assert.equal(r.private_material_negative_scan.passed,true);
 assert.equal(r.cleanup.inputs_unchanged,true);
 assert.equal(r.cleanup.backend_exit.valid,true);assert.equal(r.cleanup.http_closed,true);
 assert.equal(new Set(r.clients.map(c=>c.pid)).size,2);
 for(const c of r.clients)for(const k of ['leave_packet','left_room_readback','socket_closed','browser_process_exited'])assert.equal(c.cleanup[k],true);
 const states=JSON.parse(gunzipSync(fs.readFileSync(path.join(root,label,'states.json.gz'))));
 const proofs=[];
 for(const match of [...new Set(states.flatMap(c=>c.frames.map(f=>f.match)))]){
  // Messages continue arriving during performance capture and normal Leave.
  // Verify the ENTIRE retained shared stream, then reproduce the earlier
  // report checkpoint at its recorded last tick without relabeling that tail.
  const full=assertSharedExplosionEvidence(...states.map(c=>c.frames.filter(f=>f.match===match).map(f=>f.frame)));
  shared+=full.shared_frames;
  const checkpoint=r.matches.find(p=>p.match===match);
  assert(checkpoint,'every retained match has a report checkpoint');
  const p=assertSharedExplosionEvidence(...states.map(c=>c.frames.filter(f=>f.match===match && f.frame.tick<=checkpoint.last_tick).map(f=>f.frame)));
  proofs.push({match,...p});recordedShared+=p.shared_frames;
 }
 assert.deepEqual(canonical(proofs),canonical(r.matches),'recomputed report checkpoint evidence');
 assert(proofs.some(p=>p.complete),'shared primary/wall/prop-chain/removal sequence');
 if(label==='after-q0'){assert.equal(r.quality,0);assert.equal(r.mode,'six-screenshot gate');assert.equal(r.screenshots.length,6);}
 else {assert.equal(r.quality,6);assert.match(r.mode,/not the six-screenshot/);assert(r.sequence.frames.length>=3);}
 const sources=JSON.parse(gunzipSync(fs.readFileSync(path.join(root,label,'source-inputs.json.gz'))));
 for(const [n,text] of Object.entries(sources)){
  assert.equal(sha(Buffer.from(text)),r.hashes[n].sha256,'archived source '+n);
  if(process.argv.includes('--checkout')){assert.equal(sha(fs.readFileSync(path.join(repo,n))),r.hashes[n].sha256,'checkout '+n);compared++;}
 }
 for(const image of r.screenshots)assert.equal(sha(fs.readFileSync(path.join(root,label,image.file))),image.sha256);
 if(process.argv.includes('--checkout'))for(const [n,m] of Object.entries(r.hashes).filter(([n])=>n.startsWith('dist/web/')||n.endsWith('/ryggattack-server'))){assert.equal(sha(fs.readFileSync(path.resolve(repo,n))),m.sha256,'compiled artifact '+n);compared++;}
}
assert.equal(json('before/results.json').status,'failed','historical failure remains immutable');
assert.equal(json('reference/results.json').quality,'6');
console.log(JSON.stringify({verified_files:Object.keys(manifest).length,shared_frames_recomputed:shared,recorded_checkpoint_shared_frames:recordedShared,checkout_paths_compared:compared,images:'human review is separate',scope:'no network, browser, compiler, or gameplay mutation'}));
