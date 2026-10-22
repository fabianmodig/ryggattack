import test from 'node:test';import assert from 'node:assert/strict';import {readFileSync,existsSync} from 'node:fs';
// Static packaging contract, complemented by the real WASM/browser artifact gate.
test('web build packages every local entry-module dependency including nested HUD imports',()=>{
 const script=readFileSync(new URL('./build-web.sh',import.meta.url),'utf8');
 const copy=script.split('\n').find(line=>line.startsWith('cp web/index.html '));assert(copy);
 const pending=['index.html'],seen=new Set();
 while(pending.length){
  const name=pending.pop();if(seen.has(name))continue;seen.add(name);
  const file=new URL('../web/'+name,import.meta.url);assert(existsSync(file),name);
  const source=readFileSync(file,'utf8');
  for(const m of source.matchAll(/\bfrom\s*["']\.\/([^"']+\.mjs)["']/g))pending.push(m[1]);
  assert(copy.split(/\s+/).includes('web/'+name),'missing copied entry dependency: '+name);
 }
 assert(seen.has('online-hud.mjs')&&seen.has('online-hud-bridge.mjs'));
});
