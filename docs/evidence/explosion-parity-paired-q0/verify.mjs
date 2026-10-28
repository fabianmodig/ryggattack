import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import zlib from 'node:zlib';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const files=(dir=root)=>fs.readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name)).flatMap(e=>{assert(!e.isSymbolicLink());const p=path.join(dir,e.name);return e.isDirectory()?files(p):[path.relative(root,p)];});
const listed=files().filter(p=>p!=='manifest.json');
const manifest=JSON.parse(fs.readFileSync(path.join(root,'manifest.json')));
assert.deepEqual(listed,manifest.map(m=>m.file));
for(const item of manifest)assert.equal(sha(fs.readFileSync(path.join(root,item.file))),item.sha256,item.file);
const json=p=>JSON.parse(fs.readFileSync(path.join(root,p)));
const provenance=json('provenance.json');assert.equal(provenance.quality,0);assert.deepEqual(provenance.viewport,[960,480]);assert.equal(provenance.frames.length,10);assert.equal(provenance.clips.length,5);
for(const name of ['offline-slow','offline-normal']){const r=json(name+'/report.json');assert.equal(r.quality,0);assert.deepEqual(r.viewport,[960,480]);assert.equal(r.artifacts_unchanged,true);assert.deepEqual(r.errors,[]);assert.equal(r.hashes['ryggattack_bg.wasm'],'da79dd7b94ca815fe42ad092cc36b465faa865208b66bf50460adcd50ab67173');}
let states=0;
for(const name of ['online-slow','online-normal','two-human-normal']){
 const r=json(name+'/report.json');assert.equal(r.status,'captured-review-required');assert.deepEqual(r.errors,[]);assert(r.cleanup.inputs_unchanged&&r.cleanup.http_closed&&r.cleanup.backend_exit.valid);
 for(const c of r.clients){assert(c.cleanup.left_room_readback&&c.cleanup.browser_exit.valid&&c.cleanup.page_closed);assert.equal(c.cleanup.readback.canvas[0].width,960);assert.equal(c.cleanup.readback.canvas[0].height,480);}
 const data=JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(root,name,'states.json.gz'))));
 states+=data.reduce((n,c)=>n+c.frames.length,0);
 // Retained client artifact must be the original frozen candidate.
 const old=JSON.parse(fs.readFileSync(path.join(root,'../explosion-parity-2026-10-28/after-q0/results.json')));
 for(const p of ['dist/web/ryggattack_bg.wasm','dist/web/ryggattack.js','dist/web/index.html','dist/web/rooms.mjs'])assert.equal(r.hashes[p].sha256,old.hashes[p].sha256,p);
}
const trace=json('online-slow/report.json').relay_trace,delivered=trace.filter(t=>t.delivered_ms!==undefined);
assert.equal(delivered.length,144);assert(delivered.every(t=>t.sha256===t.delivered_sha256));assert(delivered.every((t,i)=>i===0||t.tick>delivered[i-1].tick));
assert(fs.readFileSync(path.join(root,'harness/online-explosion-slow-capture.mjs.txt'),'utf8').includes('sequence && quality === 0'));
assert.equal(json('history/remote-video-api-failure.json').status,'failed');assert.equal(json('history/one-bot-guard-failure.json').status,'failed');
console.log(JSON.stringify({status:'passed',manifested_files:manifest.length,frames:provenance.frames.length,clips:provenance.clips.length,retained_states:states,delivered_byte_identical_fifo_packets:delivered.length,unchanged_candidate_artifacts:4}));
