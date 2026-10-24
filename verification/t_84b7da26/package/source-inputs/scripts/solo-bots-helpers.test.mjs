import test from 'node:test';import assert from 'node:assert/strict';
import fs from 'node:fs';import path from 'node:path';import os from 'node:os';
import {spawnSync} from 'node:child_process';import {fileURLToPath} from 'node:url';
import {parseSoloArgs} from './solo-bots-helpers.mjs';
test('visible DOM credentials block screenshots before capture',async()=>{
 const {capturePrivateScreenshot,PrivacyViolation}=await import('./solo-bots-helpers.mjs');
 const calls=[],page={locator:selector=>{assert.equal(selector,'body');return {innerText:async()=>{calls.push('dom');return 'visible synthetic-credential';}};}};
 await assert.rejects(()=>capturePrivateScreenshot(page,new Set(['synthetic-credential']),async()=>calls.push('capture')),PrivacyViolation);
 assert.deepEqual(calls,['dom']);
 page.locator=()=>({innerText:async()=>{calls.push('safe-dom');return 'safe public text';}});
 await capturePrivateScreenshot(page,new Set(['synthetic-credential']),async()=>calls.push('capture'));
 assert.deepEqual(calls,['dom','safe-dom','capture']);
});

test('publishing scans every artifact and purges earlier screenshots on privacy failure',async()=>{
 const {publishSoloEvidence}=await import('./solo-bots-helpers.mjs');
 const scratch=process.env.TMPDIR??path.join(os.homedir(),'.hermes/cache/scratch');
 fs.mkdirSync(scratch,{recursive:true});
 const root=fs.mkdtempSync(path.join(scratch,'solo-privacy-test-'));
 try {
  for(const reason of ['dom','text','nested','binary','evidence','key','unicode']) {
   const out=path.join(root,reason);fs.mkdirSync(out);fs.writeFileSync(path.join(out,'earlier.png'),Buffer.from([137,80,78,71]));
   const secret=reason==='unicode'?'synthetic-å-credential':'synthetic-credential',secrets=new Set([secret]);
   if(reason==='text'||reason==='unicode')fs.writeFileSync(path.join(out,'notes.txt'),'leaked '+secret);
   if(reason==='nested'){fs.mkdirSync(path.join(out,'nested'));fs.writeFileSync(path.join(out,'nested/notes.txt'),secret);}
   if(reason==='binary')fs.writeFileSync(path.join(out,'extra.bin'),Buffer.concat([Buffer.from([0,255]),Buffer.from(secret)]));
   // Source snapshots are scanned too. Assemble the negative fixture only at
   // execution time, without exempting source files from publication checks.
   if(reason==='key')fs.writeFileSync(path.join(out,'notes.txt'),['-----BEGIN ', 'PRIVATE KEY', '-----'].join(''));
   const evidence={status:'passed',note:reason==='evidence'?secret:'safe'};
   assert.equal(publishSoloEvidence(out,evidence,secrets,{privacyFailed:reason==='dom'}),false,reason);
   assert.deepEqual(fs.readdirSync(out),['results.json'],reason);
   assert.deepEqual(JSON.parse(fs.readFileSync(path.join(out,'results.json'),'utf8')),{status:'failed',failure:'private-material scan failed; artifacts withheld'});
  }
  const out=path.join(root,'safe');fs.mkdirSync(out);fs.writeFileSync(path.join(out,'public.txt'),'public text');
  assert.equal(publishSoloEvidence(out,{status:'passed'},new Set(['synthetic-credential'])),true);
  assert.deepEqual(fs.readdirSync(out).sort(),['public.txt','results.json','sha256.json']);
  const proof=JSON.parse(fs.readFileSync(path.join(out,'results.json'),'utf8')).private_material_negative_scan;
  assert.equal(proof.screenshot_text_check,'visible DOM before capture; binary scans are not OCR');
 } finally {fs.rmSync(root,{recursive:true,force:true});}
});

test('privacy fixture initializes missing HOME and TMPDIR scratch parents',()=>{
 const base=process.env.TMPDIR??path.join(os.homedir(),'.hermes/cache/scratch');fs.mkdirSync(base,{recursive:true});
 const sandbox=fs.mkdtempSync(path.join(base,'solo-scratch-regression-'));
 try {
  for(const mode of ['home','tmpdir']) {
   const env={...process.env,HOME:path.join(sandbox,mode,'home')};delete env.TMPDIR;
   // A nested --test invocation must not inherit the outer runner's child marker.
   delete env.NODE_TEST_CONTEXT;
   if(mode==='tmpdir')env.TMPDIR=path.join(sandbox,mode,'custom-scratch');
   const scratch=env.TMPDIR??path.join(env.HOME,'.hermes/cache/scratch');assert(!fs.existsSync(scratch));
   const result=spawnSync(process.execPath,['--test','--test-name-pattern=publishing scans every artifact',fileURLToPath(import.meta.url)],{encoding:'utf8',timeout:10000,env});
   assert.equal(result.status,0,result.stdout+result.stderr);assert.match(result.stdout,/# tests 1\b/);assert.match(result.stdout,/# pass 1\b/);
   assert(fs.statSync(scratch).isDirectory());assert.deepEqual(fs.readdirSync(scratch),[]);
  }
 } finally {fs.rmSync(sandbox,{recursive:true,force:true});}
});

test('observed input samples must satisfy the selected bot steering and fire policy',async()=>{
 const {recordSoloInput,hasSoloPolicyInput}=await import('./solo-bots-helpers.mjs');
 const inputs={count:0,firing_count:0,left_fire_count:0,neutral_fire_count:0};
 recordSoloInput(inputs,{horizontal:-1,vertical:0,fire:false});
 recordSoloInput(inputs,{horizontal:1,vertical:0,fire:true});
 for(const bots of [1,2,3])assert.equal(hasSoloPolicyInput(bots,inputs),false);
 recordSoloInput(inputs,{horizontal:-1,vertical:0,fire:true});
 assert.equal(hasSoloPolicyInput(1,inputs),false);
 for(const bots of [2,3])assert.equal(hasSoloPolicyInput(bots,inputs),true);
 recordSoloInput(inputs,{horizontal:0,vertical:1,fire:true});assert.equal(hasSoloPolicyInput(1,inputs),false);
 recordSoloInput(inputs,{horizontal:0,vertical:0,fire:true});assert.equal(hasSoloPolicyInput(1,inputs),true);
 assert.deepEqual(inputs,{count:5,firing_count:4,left_fire_count:1,neutral_fire_count:1});
});

test('solo help and invalid CLI are inert with a missing Playwright module',()=>{
 const script=fileURLToPath(new URL('./solo-bots-smoke.mjs',import.meta.url));
 const preload='data:text/javascript,'+encodeURIComponent("import fs from 'node:fs';import cp from 'node:child_process';import {syncBuiltinESMExports} from 'node:module';const forbidden=()=>process.exit(99);fs.mkdirSync=forbidden;fs.writeFileSync=forbidden;cp.execFile=forbidden;globalThis.fetch=forbidden;globalThis.WebSocket=class{constructor(){forbidden()}};syncBuiltinESMExports();");
 for(const [args,status] of [[['--help'],0],[['-h'],0],[[],2],[['--bots','1'],2],[['--live','--bots','0'],2],[['--live','--bots','4'],2],[['--live','--bots'],2],[['--live','--live'],2],[['--live','--url','https://example.invalid'],2]]) {
  const result=spawnSync(process.execPath,['--import',preload,script,...args],{encoding:'utf8',timeout:3000,env:{...process.env,PLAYWRIGHT_NODE_MODULES:'/nonexistent/never-import-playwright.json'}});
  assert.equal(result.status,status,result.stderr);assert.match(result.stdout+result.stderr,/Usage:/);
  assert.doesNotMatch(result.stdout+result.stderr,/Cannot find module|ERR_MODULE_NOT_FOUND|ENOENT/);
 }
});

test('unchanged default creates the original three-bot configuration',()=>assert.deepEqual(parseSoloArgs(['--live']),{bots:3}));
test('explicit bot configuration covers all supported non-practice rosters',()=>{
 for(const bots of [1,2,3])for(const args of [['--live','--bots',String(bots)],['--bots',String(bots),'--live']])assert.deepEqual(parseSoloArgs(args),{bots});
});
test('authorization and bot count fail closed before any live side effects',()=>{
 for(const args of [[],['--bots','1'],['--live','--live'],['--live','--bots'],['--live','--bots','0'],['--live','--bots','4'],['--live','--bots','1.0'],['--live','--bots','1','--bots','2'],['--live','--url','https://example.invalid']])assert.throws(()=>parseSoloArgs(args));
});
