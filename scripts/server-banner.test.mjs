import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import path from 'node:path';
const child=spawn(path.resolve('target/debug/ryggattack-server'),['127.0.0.1:0']);
try{
 const banner=await new Promise((resolve,reject)=>{child.stdout.once('data',b=>resolve(String(b)));child.once('error',reject);setTimeout(()=>reject(Error('banner timeout')),5000).unref();});
 assert(banner.includes('private rooms'),'banner must describe current room service');
 const event=JSON.parse(banner);assert.equal(event.event,'listening');assert(event.service.includes('secure reconnect'));assert(event.service.includes('no accounts'));
 console.log('private-room server banner passed');
}finally{child.kill();}
