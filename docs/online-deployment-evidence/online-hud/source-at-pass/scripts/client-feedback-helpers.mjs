import assert from 'node:assert/strict';

export function requireLocalDockerEndpoint(endpoint) {
 assert(typeof endpoint==='string'&&/^unix:\/\/\/[^\s?#\0]+$/.test(endpoint),'Docker endpoint must use an absolute local Unix socket');
 return endpoint;
}
export function localEngineEndpoint(engine,env) {
 assert(!env.CONTAINER_HOST&&!env.CONTAINER_CONNECTION,'remote Podman forbidden');
 const host=env.DOCKER_HOST?requireLocalDockerEndpoint(env.DOCKER_HOST):null;
 if(engine!=='docker')return null;
 // DOCKER_CONTEXT overrides even an explicit inherited DOCKER_HOST. Reject it
 // before context discovery, output creation, browser imports, or daemon access.
 assert(!env.DOCKER_CONTEXT,'Docker context override forbidden');
 return host;
}

export function localEngineArgs(engine,endpoint,argv) {
 assert(['podman','docker'].includes(engine),'unsupported container engine');
 return engine==='podman'?['--remote=false',...argv]:['--host',requireLocalDockerEndpoint(endpoint),...argv];
}

export function requireZeroCapabilities(status) {
 const result={};
 for(const name of ['CapEff','CapBnd','CapAmb']) {
  const value=new RegExp('^'+name+':\\s+([0-9a-fA-F]{16})$','m').exec(status)?.[1];
  assert.equal(value,'0000000000000000',name+' must exist and contain no capabilities');result[name]=value;
 }
 return result;
}

export const usage='Usage: node scripts/client-feedback-smoke.mjs --local [--engine podman|docker] [--web-image IMAGE] [--server-image IMAGE] [--timeout-ms 120000] [--startup-ms 30000]\nOnly isolated, already-local published images; never deploys, pulls, builds, or contacts the public service.';
export function parseLocalArgs(args) {
  const c={engine:'podman',webImage:'git.modig.online/fabianmodig/ryggattack/web:sha-205d048',serverImage:'git.modig.online/fabianmodig/ryggattack/server:sha-205d048',timeoutMs:120000,startupMs:30000};
  const keys={'--engine':'engine','--web-image':'webImage','--server-image':'serverImage','--timeout-ms':'timeoutMs','--startup-ms':'startupMs'},seen=new Set();
  for(let i=0;i<args.length;i++) {
    const flag=args[i];assert(!seen.has(flag),'duplicate flag');seen.add(flag);
    if(flag==='--local')continue;
    assert(Object.hasOwn(keys,flag),usage);const value=args[++i];assert(value&&!value.startsWith('-'),'flag value required');
    const key=keys[flag];
    if(key.endsWith('Ms')) {assert(/^\d+$/.test(value));c[key]=Number(value);assert(Number.isSafeInteger(c[key])&&c[key]>=1000&&c[key]<=600000,'budget must be 1000..600000 ms');}
    else c[key]=value;
  }
  assert(seen.has('--local'),usage);assert(['podman','docker'].includes(c.engine),'unsupported container engine');
  for(const value of [c.webImage,c.serverImage])assert(/^[a-zA-Z0-9][a-zA-Z0-9./_:@-]+$/.test(value),'invalid image reference');
  return c;
}
export function localPort(text) {
  const m=/^127\.0\.0\.1:(\d+)$/.exec(text.trim());assert(m,'container publication must be IPv4 loopback only');
  const n=Number(m[1]);assert(n>0&&n<=65535,'invalid port');return n;
}
// Serialized by Playwright into the real page; deliberately never reads diagnostic globals.
export function noticeMatches({source,flags=''}) {
  const e=document.getElementById('room-notice');if(!e)return false;
  if(e.closest('details')?.open===false)return false;
  const style=getComputedStyle(e),r=e.getBoundingClientRect();
  if(style.display==='none'||style.visibility!=='visible'||Number(style.opacity)===0||!e.getClientRects().length||r.width<=0||r.height<=0)return false;
  if(r.bottom<=0||r.right<=0||r.top>=innerHeight||r.left>=innerWidth)return false;
  return new RegExp(source,flags).test(e.textContent??'');
}
export function requirePrivateScan(text,secrets) {
  for(const s of secrets)assert(typeof s==='string'&&s.length>0&&!text.includes(s),'private credential found in evidence');
  assert(!/-----BEGIN (?:RSA |EC )?PRIVATE KEY-----/.test(text),'private TLS key found in evidence');
}
