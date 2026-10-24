import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
const hash = p => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const root = '/home/hermes/.hermes/artifacts/ryggattack/wasm-packaging-20261024/bundle';
const retained = JSON.parse(fs.readFileSync('verification/t_621f9087-run113/retained-verification.json'));
execFileSync('sha256sum',['-c',retained.source_manifest],{stdio:'pipe'});
assert.equal(crypto.createHash('sha256').update(execFileSync('git',['diff','--binary'])).digest('hex'),retained.tracked_candidate_patch_sha256);
for (const e of retained.native_artifacts) assert.equal(hash(e.path),e.sha256);
for (const e of retained.wasm.outputs) { assert.equal(hash(path.join(root,e.path)),e.sha256); assert.equal(fs.statSync(path.join(root,e.path)).size,e.bytes); }
const old = 'dist/web-before-t_84b7da26';
assert(!fs.existsSync(old),'preserved old bundle path already exists');
if(fs.existsSync('dist/web')) fs.renameSync('dist/web',old);
fs.cpSync(root,'dist/web',{recursive:true,preserveTimestamps:true,errorOnExist:true,force:false});
for (const e of retained.wasm.outputs) assert.equal(hash(path.join('dist/web',e.path)),e.sha256);
console.log(JSON.stringify({status:'passed',source_manifest:retained.source_manifest,source_manifest_sha256:hash(retained.source_manifest),tracked_patch_sha256:retained.tracked_candidate_patch_sha256,bundle:root,previous_bundle:old,staged_bundle:'dist/web',outputs:retained.wasm.outputs,native:retained.native_artifacts,builds_run:0},null,2));
