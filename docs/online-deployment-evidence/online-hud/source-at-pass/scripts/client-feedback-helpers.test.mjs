import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const script=fileURLToPath(new URL('./client-feedback-smoke.mjs',import.meta.url));
// Any accidental side effect fails before it can touch disk or execute an engine.
const inertPreload='data:text/javascript,'+encodeURIComponent("import fs from 'node:fs';import cp from 'node:child_process';import {syncBuiltinESMExports} from 'node:module';const forbidden=()=>process.exit(99);fs.mkdirSync=forbidden;fs.writeFileSync=forbidden;cp.execFile=forbidden;globalThis.fetch=forbidden;globalThis.WebSocket=class{constructor(){forbidden()}};syncBuiltinESMExports();");
import {parseLocalArgs, noticeMatches, requirePrivateScan, localPort, requireZeroCapabilities} from './client-feedback-helpers.mjs';

test('Docker context overrides cannot bypass a local unix DOCKER_HOST',async()=>{
 const {localEngineEndpoint}=await import('./client-feedback-helpers.mjs');
 assert.throws(()=>localEngineEndpoint('docker',{DOCKER_CONTEXT:'remote-production',DOCKER_HOST:'unix:///run/user/1000/docker.sock'}),/Docker context override forbidden/);
 assert.throws(()=>localEngineEndpoint('docker',{DOCKER_CONTEXT:'default'}),/Docker context override forbidden/);
 assert.equal(localEngineEndpoint('docker',{DOCKER_HOST:'unix:///run/user/1000/docker.sock'}),'unix:///run/user/1000/docker.sock');
 assert.equal(localEngineEndpoint('docker',{}),null);
 for(const host of ['tcp://host:2375','ssh://host','unix://','unix://remote/path','unix:///'])assert.throws(()=>localEngineEndpoint('docker',{DOCKER_HOST:host}));
});

test('inherited remote engine selectors fail closed even for Podman',async()=>{
 const {localEngineEndpoint}=await import('./client-feedback-helpers.mjs');
 for(const engine of ['podman','docker'])for(const env of [{CONTAINER_HOST:'ssh://remote'},{CONTAINER_CONNECTION:'remote'},{DOCKER_HOST:'tcp://remote:2375'}])assert.throws(()=>localEngineEndpoint(engine,env));
 assert.equal(localEngineEndpoint('podman',{DOCKER_HOST:'unix:///run/user/1000/docker.sock'}),null);
});

test('every engine command pins its validated local transport',async()=>{
 const {localEngineArgs}=await import('./client-feedback-helpers.mjs');
 for(const argv of [['info'],['image','inspect','fixture'],['run','fixture'],['exec','fixture','cat','/proc/1/status'],['rm','-f','fixture']]) {
  assert.deepEqual(localEngineArgs('docker','unix:///run/user/1000/docker.sock',argv),['--host','unix:///run/user/1000/docker.sock',...argv]);
  assert.deepEqual(localEngineArgs('podman',null,argv),['--remote=false',...argv]);
 }
 for(const endpoint of [null,'tcp://remote:2375','unix://remote/path'])assert.throws(()=>localEngineArgs('docker',endpoint,['info']));
});

test('Docker inherited context rejection is process-level and precedes output or dependency effects',()=>{
 const marker='synthetic-private-marker';
 for(const overrides of [
  {DOCKER_CONTEXT:'remote-'+marker,DOCKER_HOST:'unix:///run/user/1000/docker.sock'},
  {DOCKER_CONTEXT:'remote-'+marker,DOCKER_HOST:''},
  {DOCKER_CONTEXT:'',DOCKER_HOST:'ssh://user:'+marker+'@remote'},
  {DOCKER_CONTEXT:'',DOCKER_HOST:'unix://remote/'+marker},
 ]) {
  const result=spawnSync(process.execPath,['--import',inertPreload,script,'--local','--engine','docker'],{encoding:'utf8',timeout:3000,env:{...process.env,NODE_TLS_REJECT_UNAUTHORIZED:'1',CONTAINER_HOST:'',CONTAINER_CONNECTION:'',...overrides,PLAYWRIGHT_NODE_MODULES:'/nonexistent/never-import-playwright.json'}});
  assert.equal(result.status,2);assert.match(result.stderr,/Local engine authorization failed/);
  assert(!result.stdout.includes(marker)&&!result.stderr.includes(marker),'guard must not echo raw context or endpoint');
  assert.doesNotMatch(result.stderr,/ENOENT|Cannot find module|ERR_MODULE_NOT_FOUND/);
 }
});

test('actual process effective/bounding/ambient capability sets must all be zero',()=>{
 const zero='CapEff:\t0000000000000000\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\n';
 assert.deepEqual(requireZeroCapabilities(zero),{CapEff:'0000000000000000',CapBnd:'0000000000000000',CapAmb:'0000000000000000'});
 for(const name of ['CapEff','CapBnd','CapAmb']) {
  assert.throws(()=>requireZeroCapabilities(zero.replace(name+':\t0000000000000000',name+':\t0000000000000001')));
  assert.throws(()=>requireZeroCapabilities(zero.split('\n').filter(line=>!line.startsWith(name+':')).join('\n')));
 }
});

test('local authorization is mandatory; unknown and unsafe arguments fail closed', () => {
  for (const args of [[], ['--live'], ['--local','--wat'], ['--local','--web-image'], ['--local','--timeout-ms','0'], ['--local','--engine','sh'], ['--local','--local']]) assert.throws(()=>parseLocalArgs(args));
  const c=parseLocalArgs(['--local']);
  assert.equal(c.engine,'podman');assert.match(c.webImage,/:sha-205d048$/);assert.match(c.serverImage,/:sha-205d048$/);
  assert.equal(parseLocalArgs(['--local','--timeout-ms','120000']).timeoutMs,120000);
});

test('only explicit IPv4 loopback published ports are accepted', () => {
  assert.equal(localPort('127.0.0.1:34567\n'),34567);
  for (const value of ['0.0.0.0:80','[::]:80','example.com:80','127.0.0.1:0','127.0.0.1:65536','127.0.0.1:80\n0.0.0.0:80']) assert.throws(()=>localPort(value));
});

test('DOM text, not the diagnostic global, is authoritative; hidden notices fail', () => {
  const saved={document:globalThis.document,getComputedStyle:globalThis.getComputedStyle,innerWidth:globalThis.innerWidth,innerHeight:globalThis.innerHeight};
  let closed=false,hidden=false,text='Backend unavailable; join again';
  const details={open:true};
  const node={get textContent(){return text;},closest:()=>closed?{open:false}:details,getBoundingClientRect:()=>({width:200,height:25,top:1,left:1,bottom:26,right:201}),getClientRects:()=>hidden?[]:[{}]};
  globalThis.document={getElementById:()=>node};globalThis.getComputedStyle=()=>({visibility:hidden?'hidden':'visible',display:'block',opacity:'1'});globalThis.innerWidth=640;globalThis.innerHeight=480;
  try {
    assert.equal(noticeMatches({source:'unavailable',flags:'i'}),true);
    text='Link copied';globalThis.ryggattackRoom=JSON.stringify({status:'Backend unavailable'});
    assert.equal(noticeMatches({source:'unavailable',flags:'i'}),false);
    text='Backend unavailable';closed=true;assert.equal(noticeMatches({source:'unavailable',flags:'i'}),false);
    closed=false;hidden=true;assert.equal(noticeMatches({source:'unavailable',flags:'i'}),false);
  } finally {Object.assign(globalThis,saved);delete globalThis.ryggattackRoom;}
});

test('CLI authorization and help are inert even without any browser dependency', () => {
  for(const [args,status] of [[[],2],[['--live'],2],[['--local','--wat'],2],[['--local','--timeout-ms','0'],2],[['--help'],0]]) {
    const result=spawnSync(process.execPath,[ '--import',inertPreload,script,...args],{encoding:'utf8',timeout:3000,env:{...process.env,PLAYWRIGHT_NODE_MODULES:'/nonexistent/never-import-playwright.json'}});
    assert.equal(result.status,status,result.stderr);assert.match(result.stdout+result.stderr,/Usage:/);
    assert.doesNotMatch(result.stdout+result.stderr,/ERR_MODULE_NOT_FOUND|Cannot find module|ENOENT/);
  }
});

test('credential negative scan rejects exact known secrets and private key evidence', () => {
  assert.doesNotThrow(()=>requirePrivateScan('safe text',new Set(['memory-only-secret'])));
  assert.throws(()=>requirePrivateScan('oops memory-only-secret',new Set(['memory-only-secret'])));
  assert.throws(()=>requirePrivateScan('-----BEGIN PRIVATE KEY-----',new Set()));
});
