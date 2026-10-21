import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
const html=fs.readFileSync('web/pages-redirect.html','utf8');
const code=html.match(/<script>([\s\S]*?)<\/script>/)[1];
for(const [search,expected] of [['',''],['?quality=0','?quality=0'],['?room=0123456789abcdef0123&name=A','?room=0123456789abcdef0123&name=A&online=wss%3A%2F%2Fryggattack.modig.online%2Fws'],['?online=ws%3A%2F%2Fevil.example','?online=wss%3A%2F%2Fryggattack.modig.online%2Fws']]){
 let redirected;vm.runInNewContext(code,{URL,location:{search,replace:url=>redirected=url}});
 assert.equal(redirected,'https://ryggattack.modig.online/'+expected);
}
console.log('Pages canonical redirect preserves room/settings, replaces obsolete backends, retains offline entry');
