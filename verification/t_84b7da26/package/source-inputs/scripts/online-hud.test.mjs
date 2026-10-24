import test from 'node:test';
import assert from 'node:assert/strict';
const hud = await import('../web/online-hud.mjs').catch(error => {
  if (error.code === 'ERR_MODULE_NOT_FOUND') return {};
  throw error;
});
// Minimal DOM contract double: real-browser layout/input verification belongs to integration.
class Element {
  constructor(tag,doc) { this.tagName=tag.toUpperCase(); this.ownerDocument=doc; this.children=[]; this.attributes={}; this.hidden=false; this.open=false; this._text=''; }
  append(...nodes) { for (const node of nodes) { node.parentNode=this; this.children.push(node); } }
  replaceChildren(...nodes) { this.children.forEach(node => {node.parentNode=null;}); this.children=[]; this._text=''; this.append(...nodes); }
  set textContent(text) { this.replaceChildren(); this._text=String(text); }
  get textContent() { return this._text + this.children.map(node=>node.textContent).join(''); }
  set innerHTML(_) { throw Error('innerHTML is forbidden'); }
  setAttribute(name,value) { this.attributes[name]=String(value); }
  getAttribute(name) { return this.attributes[name] ?? null; }
  remove() { if(this.parentNode) this.parentNode.children=this.parentNode.children.filter(node=>node!==this); this.parentNode=null; }
  focus() { this.ownerDocument.activeElement=this; }
}
function dom() {
  const doc={createElement(tag) {return new Element(tag,doc);},activeElement:null};
  return {doc,host:doc.createElement('div')};
}
function find(root,tag) { return root.tagName===tag.toUpperCase() ? root : root.children.map(child=>find(child,tag)).find(Boolean); }

test('DOM mount/update is compact, text-only, collapsible, focus-neutral and removes old rows/outcomes', () => {
  assert.equal(typeof hud.mountOnlineHud,'function');
  const {doc,host}=dom(), canvas=doc.createElement('canvas'), controls=doc.createElement('input');
  host.append(canvas,controls); canvas.focus();
  const view=hud.mountOnlineHud(host);
  const root=view.element;
  assert.equal(root.getAttribute('aria-label'),'Online game status');
  const details=find(root,'details'), summary=find(root,'summary');
  assert.equal(details.open,false);
  assert.equal(summary.textContent,'Diagnostics');
  const input=freeze(snapshot({winner:0,names:['<img src=x onerror=alert(1)>',null,null,null]}, {credential:'SECRET_CANARY',reconnect:{credential:'SECRET_CANARY'}}));
  const before=JSON.stringify(input);
  view.update(input);
  assert.match(root.textContent,/<img src=x onerror=alert\(1\)> wins the match/);
  assert.equal(find(root,'img'),undefined);
  assert.equal(root.textContent.includes('SECRET_CANARY'),false);
  assert.equal(JSON.stringify(input),before);
  assert.equal(doc.activeElement,canvas);
  details.open=true; summary.focus();
  view.update(snapshot({carts:[cart(1,{alive:false,speed_seconds:0.01})]}));
  assert.equal(find(root,'details'),details);
  assert.equal(find(root,'summary'),summary);
  assert.equal(details.open,true);
  assert.equal(doc.activeElement,summary);
  assert.equal(root.textContent.includes('wins the match'),false);
  assert.equal(root.textContent.includes('Ada'),false);
  assert.match(root.textContent,/Bot 2.*OUT.*Speed 0.1s/);
  controls.focus();
  view.update(snapshot({}, {status:'Connection failed',notice:'Leave and rejoin',lobby:null,seat:null}));
  assert.equal(doc.activeElement,controls);
  assert.match(root.textContent,/Connection failed/);
  assert.match(root.textContent,/Leave and rejoin/);
  assert.equal(root.textContent.includes('Bot 2'),false);
  const css=find(root,'style').textContent;
  assert.match(css,/@media/);
  assert.match(css,/pointer-events:\s*none/);
  assert.match(css,/pointer-events:\s*auto/);
  assert.equal(find(root,'button'),undefined,'room controls stay separate');
  view.destroy(); view.destroy();
  assert.deepEqual(host.children,[canvas,controls]);
});

const build = input => { assert.equal(typeof hud.buildOnlineHudModel, 'function', 'HUD model export exists'); return hud.buildOnlineHudModel(input); };

for (const [label, boundary] of [
  ['room change', old => ({...old,lobby:{id:8,phase:'Running'}})],
  ['Lobby', old => ({...old,lobby:{id:7,phase:'Lobby'}})],
  ['Countdown', old => ({...old,lobby:{id:7,phase:{Countdown:{ticks_left:64}}}})],
  ['reset', old => ({...old,frame:null})],
  ['leave', old => ({...old,lobby:null,seat:null})],
]) test(`DOM staleness: ${label} clears old outcome until a fresh frame`, () => {
  const {host}=dom(), view=hud.mountOnlineHud(host);
  const old=freeze(snapshot({winner:0,round_finished:true}));
  view.update(old);
  assert.match(view.element.textContent,/Ada wins the match/);
  const changed=boundary(old);
  const before=JSON.stringify(changed);
  view.update(freeze(changed));
  assert.equal(JSON.stringify(changed),before);
  assert.equal(view.element.textContent.includes('wins the match'),false);
  const room=label==='room change' ? 8 : 7;
  view.update(freeze(snapshot(old.frame,{lobby:{id:room,phase:'Running'}})));
  assert.equal(view.element.textContent.includes('wins the match'),false,'reparsed stale frame remains suppressed');
  const fresh=snapshot({tick:11,simulation_tick:9},{lobby:{id:room,phase:'Running'}});
  view.update(freeze(fresh));
  assert.match(view.element.textContent,/Ada/);
  assert.equal(view.element.textContent.includes('wins the match'),false);
  view.update(freeze(snapshot({tick:12,winner:1},{lobby:{id:room,phase:'Running'}})));
  assert.match(view.element.textContent,/Bot 2 wins the match/);
  view.update(freeze({...fresh,frame:{...fresh.frame,tick:13}}));
  assert.equal(view.element.textContent.includes('wins the match'),false,'new round clears finished match');
  view.destroy();
});
const cart = (id, extra = {}) => ({id, active:true, alive:true, speed_seconds:0, all_hits_seconds:0, ...extra});
const snapshot = (frame = {}, extra = {}) => ({lobby:{id:7, phase:'Running'}, seat:0, status:'Online — seat 1', notice:'', menu:false, rtt_ms:12, frame:{tick:10, simulation_tick:8, names:['Ada',null,'Lin',null], wins:[2,1,0,0], carts:[cart(0),cart(1),cart(2,{active:false})], winner:null, round_finished:false, transition:0, ...frame}, ...extra});
function freeze(value) { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value); } return value; }

test('active seats have names, bot fallback, score, OUT and simultaneous positive authoritative buffs', () => {
  const input = freeze(snapshot({carts:[cart(2,{active:false}),cart(1,{alive:false}),cart(0,{speed_seconds:2.25,all_hits_seconds:0.4})]}));
  const before = JSON.stringify(input);
  const model = build(input);
  assert.deepEqual(model.players, [
    {seat:0,name:'Ada',score:2,out:false,you:true,buffs:[{kind:'speed',label:'Speed',seconds:2.25},{kind:'all-hits',label:'All-hits',seconds:0.4}]},
    {seat:1,name:'Bot 2',score:1,out:true,you:false,buffs:[]},
  ]);
  assert.equal(JSON.stringify(input),before);
  assert.equal(model.outcome,null);
});

for (const [label, frame, expected] of [
  ['no score-based winner',{wins:[99,0,0,0]},null],
  ['named match winner',{winner:0,round_finished:true},{kind:'match',seat:0,name:'Ada',text:'Ada wins the match'}],
  ['bot match winner',{winner:1},{kind:'match',seat:1,name:'Bot 2',text:'Bot 2 wins the match'}],
  ['sole survivor round',{round_finished:true,transition:2,carts:[cart(0,{alive:false}),cart(1),cart(2,{active:false})]},{kind:'round',seat:1,name:'Bot 2',text:'Bot 2 wins the round'}],
  ['no survivors draw',{round_finished:true,carts:[cart(0,{alive:false}),cart(1,{alive:false})]},{kind:'draw',text:'Round draw'}],
  ['multiple survivors unknown',{round_finished:true},null],
  ['alive before finish',{carts:[cart(0)]},null],
  ['inactive winner ignored',{winner:2},null],
  ['invalid winner ignored',{winner:9},null],
]) test(`outcome: ${label}`, () => assert.deepEqual(build(freeze(snapshot(frame))).outcome,expected));

for (const [label, extra] of [
  ['Lobby',{lobby:{id:7,phase:'Lobby'}}],
  ['Countdown',{lobby:{id:7,phase:{Countdown:{ticks_left:64}}}}],
  ['leave',{lobby:null,seat:null}],
  ['no assigned seat',{seat:null}],
  ['reset',{frame:null}],
]) test(`clear stale game: ${label}`, () => {
  const model = build(freeze(snapshot({winner:0,round_finished:true},extra)));
  assert.deepEqual(model.players,[]);
  assert.equal(model.outcome,null);
});

test('expired/invalid buffs are omitted, positive timers are never locally decremented, dead seat stays OUT', () => {
  for (const seconds of [0,-1,NaN,Infinity]) {
    const input = freeze(snapshot({carts:[cart(0,{alive:false,speed_seconds:seconds,all_hits_seconds:0.01})]}));
    assert.deepEqual(build(input).players[0].buffs,[{kind:'all-hits',label:'All-hits',seconds:0.01}]);
    assert.equal(build(input).players[0].out,true);
    assert.deepEqual(build(input),build(input));
  }
});

test('diagnostics whitelist excludes all credential/reconnect metadata; failures remain visible outside diagnostics', () => {
  const canary = 'SECRET_CANARY';
  const input = freeze(snapshot({}, {status:'Backend unavailable',notice:'Try joining again',menu:true,rtt_ms:18.5,
    credential:canary,reconnect:{token:canary},participant:canary,
    lobby:{id:7,phase:'Running',credential:canary,humans:[{reconnect_seconds:canary}]}}));
  const model = build(input);
  assert.equal(model.connection,'Backend unavailable');
  assert.equal(model.notice,'Try joining again');
  assert.equal(model.menu,true);
  assert.deepEqual(model.diagnostics,{tick:10,simulation_tick:8,rtt_ms:18.5});
  assert.equal(JSON.stringify(model).includes(canary),false);
  assert.equal(JSON.stringify(model).includes('reconnect'),false);
});
