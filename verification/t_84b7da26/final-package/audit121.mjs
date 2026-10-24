import fs from 'node:fs';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const base='verification/t_84b7da26',root='verification/rear-browser120/2026-10-24T18-18-47-479Z-4b0bcd130a67';
const r=JSON.parse(fs.readFileSync(root+'/results.json'));
assert.equal(r.status,'failed');assert.equal(r.errors.length,0);assert.equal(r.cleanup.inputs_unchanged,true);assert.equal(r.cleanup.http_closed,true);assert.equal(r.cleanup.backend_exit.valid,true);
const sources=JSON.parse(fs.readFileSync(base+'/package/source-map.json'));
let sourceCount=0;for(const e of sources)if(r.hashes[e.path]){assert.equal(r.hashes[e.path].sha256,e.sha256,e.path);sourceCount++;}
const artifacts=Object.entries(r.hashes).filter(([p])=>p.startsWith('dist/web/'));
assert.equal(artifacts.length,9);for(const [p,e]of artifacts)assert.equal(sha(fs.readFileSync(p)),e.sha256,p);
const prior=JSON.parse(fs.readFileSync(base+'/package/preflight.json'));
const analysis=JSON.parse(fs.readFileSync(base+'/rear-rail-analysis120.json'));
assert.equal(analysis.event_count,1);assert.equal(sha(fs.readFileSync(root+'/states.json')),analysis.states_sha256);
const tests=JSON.parse(fs.readFileSync(base+'/cached-check120.json'));assert.equal(tests.native_passed,19);assert.equal(tests.node.passed,143);
console.log(JSON.stringify({status:'passed',source_head:r.source_revision,source_worktree:r.source_worktree,overlapping_source_hashes:sourceCount,served_artifact_members:artifacts.length,artifact_hashes:Object.fromEntries(artifacts),original_launcher_status:r.status,original_launcher_failure:r.failure.split('\n')[0],total_ms:r.total_ms,cleanup:r.cleanup,private_material_negative_scan:r.private_material_negative_scan,independent_analysis_status:analysis.status,event_count:analysis.event_count,geometry_tests:analysis.geometry_self_tests,cached_native_passed:tests.native_passed,node_passed:tests.node.passed,scope:'Read-only provenance/geometry corroboration, no new browser/build; complete original launcher failure retained. Original compiler provenance/build durations are in frozen package.',preflight_sha256:sha(fs.readFileSync(base+'/package/preflight.json'))},null,2));
