import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
const binary=path.resolve('target/debug/ryggattack-server');
for(const origin of [undefined,'','*','null','http://ryggattack.modig.online','https://ryggattack.modig.online/','https://ryggattack.modig.online:443','https://ryggattack.modig.online,https://evil.example']){
 const env={...process.env,RYGGATTACK_MODE:'production'};delete env.RYGGATTACK_ALLOWED_ORIGINS;if(origin!==undefined)env.RYGGATTACK_ALLOWED_ORIGINS=origin;
 const r=spawnSync(binary,['0.0.0.0:0'],{env,timeout:2000,encoding:'utf8'});assert.equal(r.status,1,`unsafe origin ${origin} must fail closed before startup`);assert.match(r.stderr,/production requires exactly/);
}
for(const mode of ['','public','PRODUCTION']){
 const r=spawnSync(binary,['127.0.0.1:0'],{env:{...process.env,RYGGATTACK_MODE:mode},timeout:2000,encoding:'utf8'});assert.equal(r.status,1);assert.match(r.stderr,/invalid RYGGATTACK_MODE/);
}
console.log('Production origin/mode configuration fails closed for missing, wildcard, null, malformed and mismatched policies');
