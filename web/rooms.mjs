// Browser menu owns presentation and launch intent, never a second transport.
export function joinUrl(base, address, code) {
  const url=new URL(base); url.searchParams.set('online',address); url.searchParams.set('room',code); url.searchParams.delete('name'); return url.href;
}
export function settingsCommand(arena, target, bots) {
  const wins_target=Number(target); bots=Number(bots);
  if(!['Forest','Volcano'].includes(arena)||![1,3,5,7].includes(wins_target)||!Number.isInteger(bots)||bots<0||bots>3)throw Error('Invalid settings');
  return {Configure:{arena,wins_target,bots}};
}
export function participantStatus(h, phase) {
  if(!h.connected)return `${phase==='Running'?'Bot takeover':'Disconnected'} · reserved ${h.reconnect_seconds??0}s`;
  return phase==='Running'?'playing':h.ready?'ready':'not ready';
}
export function validLaunch(value) {
  if(!value || typeof value.address!=='string' || typeof value.name!=='string' || !value.name.trim() || value.name.length>24)return false;
  try {const u=new URL(value.address);if(!['ws:','wss:'].includes(u.protocol)||u.username||u.password||u.hash)return false;}catch{return false;}
  return value.code===null || /^[a-f0-9]{20}$/.test(value.code??'');
}
export function savedLaunch(storage, address, name, code) {
  if(!/^[a-f0-9]{20}$/.test(code??''))return null;
  try {
    const claim=JSON.parse(storage?.getItem(`ryggattack.claim.${JSON.stringify([address,code])}`)??'null');
    const launch={address,name,code};
    return validLaunch(launch)&&Number.isInteger(claim?.room)&&Number.isInteger(claim?.participant)&&Number.isInteger(claim?.seat)&&claim.seat>=0&&claim.seat<4&&/^[a-f0-9]{64}$/.test(claim.credential??'')?launch:null;
  }catch{return null;}
}
// Shared by real Bevy keyboard/gamepad forwarding and deterministic DOM tests.
export function navigateControls(root, input, back, document=root.ownerDocument) {
  if(!input.up&&!input.down&&!input.left&&!input.right&&!input.accept&&!input.start&&!input.cancel)return;
  const active=document.activeElement;
  if(input.controller&&!active?.disabled&&(input.left||input.right)) {
    const step=input.right?1:-1;
    if(active?.matches('select')) {
      const options=[...active.options].filter(o=>!o.disabled),index=options.findIndex(o=>o.value===active.value);
      if(options.length){active.value=options[(index+step+options.length)%options.length].value;active.dispatchEvent(new Event('change',{bubbles:true}));}
      return;
    }
    if(active?.type==='number') {if(step>0)active.stepUp();else active.stepDown();active.dispatchEvent(new Event('input',{bubbles:true}));return;}
  }
  if(active?.matches('input,select,textarea')) {
    if(input.cancel){active.blur();[...root.querySelectorAll('button')].find(e=>!e.disabled&&e.getClientRects().length)?.focus();}
    if(input.cancel||!input.controller)return; // text keys never activate room actions
  }
  if(input.cancel){back();return;}
  const controls=[...root.querySelectorAll('button,input,select,summary')].filter(e=>!e.disabled && e.getClientRects().length);
  if(!controls.length)return;
  let index=controls.indexOf(active);
  const step=(input.down||input.right?1:0)-(input.up||input.left?1:0);
  if(index<0)index=0;else index=(index+step+controls.length)%controls.length;
  if(step||!controls.includes(active))controls[index].focus();
  if(input.accept||input.start)controls[index].click();
}
export function installRoomControls(env=globalThis) {
  const {document,location}=env, $=id=>document.getElementById(id), params=new URLSearchParams(location.search);
  let storage;try{storage=env.sessionStorage;}catch{/* unavailable storage disables resume, not play */}
  const read=key=>{try{return storage?.getItem(key);}catch{return null;}};
  const write=(key,value)=>{try{storage?.setItem(key,value);return !!storage;}catch{return false;}};
  const remove=key=>{try{storage?.removeItem(key);}catch{/* optional */}};
  const quality=params.get('quality');
  $('quality-override').disabled=!/^[0-6]$/.test(quality??'');$('quality-override').value=quality??'';
  const site=new URL(location.href), local=['localhost','127.0.0.1','[::1]'].includes(site.hostname)&&site.protocol==='http:';
  const address=params.get('online')??(local?'ws://127.0.0.1:8787':`wss://${site.host}/ws`);
  const name=params.get('name')??read('ryggattack.playerName')??'Player', code=params.get('room')??'';
  $('backend').value=address;$('player-name').value=name;$('room-code').value=code;
  let launch=null;try{launch=JSON.parse(read('ryggattack.launch')??'null');}catch{/* invalid pending launch */}
  remove('ryggattack.launch');
  if(!validLaunch(launch))launch=savedLaunch(storage,address,name,code);
  // Rust reads this once before choosing its isolated local/online application.
  env.ryggattackLaunch=launch?JSON.stringify(launch):null;
  let screen='', room={}, pending=null, lastRaw='', lastLocal='', lastHudMenu=false;
  const notice=text=>{$('room-notice').textContent=text;};
  function show(next,focus=true) {
    if(screen===next)return;
    screen=next;$('online-entry').hidden=next==='local'||next==='playing';
    env.ryggattackBrowserMenu=!$('online-entry').hidden;
    for(const [id,value] of [['main-controls','main'],['online-setup','setup'],['room-controls','lobby'],['match-menu','match']])$(id).hidden=next!==value;
    $('menu-title').textContent=next==='main'?'RYGGATTACK':next==='lobby'?'PRIVATE ROOM':next==='match'?'ONLINE MATCH':'PLAY ONLINE';
    $('menu-subtitle').textContent=next==='main'?'A railway battle':next==='match'?'Input suspended · the server keeps running':'A railway battle · private online play';
    $('room-notice').hidden=next==='main';
    if(focus&&!$('online-entry').hidden){const section=$(next==='main'?'main-controls':next==='setup'?'online-setup':next==='lobby'?'room-controls':'match-menu');section.querySelector('button:not([disabled]),input:not([type=hidden])')?.focus();}
    if(next==='local'||next==='playing')$('ryggattack-canvas').focus();
  }
  function setBusy(busy) {
    for(const id of ['create-room','join-room','player-name','room-code','backend'])$(id).disabled=busy;
    $('setup-back').disabled=false;
    $('setup-back').textContent=busy?'CANCEL CONNECTION':'BACK';
  }
  function cleanMain() {
    env.ryggattackHud=null;env.ryggattackRoom=null;remove('ryggattack.launch');
    const url=new URL(location.href);for(const key of ['online','room','name'])url.searchParams.delete(key);
    location.assign(url.href);
  }
  function back() {
    if(screen==='setup'&&pending?.kind==='connect'&&launch){
      env.ryggattackCommand=JSON.stringify('Leave');pending={kind:'leave',at:Date.now()};notice('Canceling connection…');return;
    }
    if(screen==='setup'&&!pending){if(launch)cleanMain();else{notice('');show('main');$('play-online').focus();}}
    else if(screen==='match'){env.ryggattackResume=true;show('playing');}
    // Leaving a lobby is always the explicit Leave button, never Escape.
  }
  function send(command) {
    if(pending||env.ryggattackCommand){notice('Please wait for the previous command');return false;}
    env.ryggattackCommand=JSON.stringify(command);pending={kind:command==='Leave'?'leave':'command',at:Date.now()};
    notice(command==='Leave'?'Leaving room…':'Updating room…');return true;
  }
  function submit(join) {
    if(pending)return;
    const next={address:$('backend').value.trim(),name:$('player-name').value.trim(),code:join?$('room-code').value.trim():null};
    if(!validLaunch(next)){notice(join?'Enter a player name, a valid ws:// or wss:// backend and the 20-character lowercase room code.':'Enter a player name (1–24 characters) and a valid ws:// or wss:// backend.');return;}
    // Explicit button activation is the only way to make a fresh admission.
    if(!write('ryggattack.launch',JSON.stringify(next))){notice('Session storage is unavailable. Enable it to enter a private room safely.');return;}
    write('ryggattack.playerName',next.name);pending={kind:'connect',at:Date.now()};setBusy(true);notice('Connecting…');
    const url=new URL(location.href);url.searchParams.set('online',next.address);url.searchParams.set('name',next.name);
    if(next.code)url.searchParams.set('room',next.code);else url.searchParams.delete('room');
    location.assign(url.href);
  }
  $('play-local').onclick=()=>{env.ryggattackLocalAction='local';show('local');};
  $('play-online').onclick=()=>{notice('');show('setup');};
  $('setup-back').onclick=back;
  $('room-form').onsubmit=event=>{event.preventDefault();};
  $('create-room').onclick=()=>submit(false);$('join-room').onclick=()=>submit(true);
  $('ready-room').onclick=()=>send({Ready:!room.lobby?.humans.find(h=>h.id===room.participant)?.ready});
  $('apply-room').onclick=()=>{try{send(settingsCommand($('arena-room').value,$('target-room').value,$('bots-room').value));}catch(error){notice(error.message);}};
  $('leave-room').onclick=() => send('Leave');$('leave-match').onclick=() => send('Leave');
  $('resume-match').onclick=back;
  $('open-match-menu').onclick=()=>{env.ryggattackOpenMenu=true;show('match');};
  $('copy-room').onclick=async()=>{try{await env.navigator.clipboard.writeText($('share-room').value);notice('Link copied');}catch{$('share-room').focus();$('share-room').select();notice('Clipboard unavailable. Select and copy this link.');}};
  env.ryggattackNavigate=input=>{if(env.ryggattackBrowserMenu)navigateControls($('online-entry'),input,back,document);};
  // Keys typed into DOM controls never reach canvas shortcuts. Bevy forwards
  // keyboard/pad intent only while canvas focused; DOM keyboard uses this path.
  $('online-entry').addEventListener('keydown',event=>{
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();env.ryggattackNavigate({cancel:true});}
    else if(!event.target.matches('input,select,textarea')&&['ArrowDown','ArrowUp','ArrowLeft','ArrowRight'].includes(event.key)){
      event.preventDefault();env.ryggattackNavigate({down:event.key==='ArrowDown',up:event.key==='ArrowUp',left:event.key==='ArrowLeft',right:event.key==='ArrowRight'});
    }
  });
  if(launch){pending={kind:'connect',at:Date.now()};setBusy(true);notice('Connecting / restoring saved seat…');show('setup');}
  else show(params.has('room')||params.has('online')?'setup':'main');
  const timer=env.setInterval(()=>{
    const localState=env.ryggattackLocalScreen;
    if(!launch&&localState&&localState!==lastLocal){lastLocal=localState;if(localState==='MainMenu'){if(screen==='local')show('main');}else show('local');}
    const raw=env.ryggattackRoom;
    if(raw&&raw!==lastRaw){
      lastRaw=raw;try{room=JSON.parse(raw);}catch{return;}
      const l=room.lobby;
      if(pending?.kind==='leave'&&!l&&room.status==='Left room; create or join another'){
        pending=null;setBusy(false);notice('Left room. Your seat has been released.');show('setup');
      } else if(l){
        pending=null;setBusy(false);
        const running=l.phase==='Running', host=l.host===room.participant;
        $('room-info').textContent=`Room ${l.code} · ${typeof l.phase==='string'?l.phase:`Starting in ${(l.phase.Countdown.ticks_left/64).toFixed(1)}s`}`;
        $('room-roster').replaceChildren(...l.humans.map(h=>{const card=document.createElement('li');card.className='seat-card';card.textContent=`P${h.seat+1} · ${h.name}${h.id===room.participant?' (you)':''}${h.id===l.host?' · HOST':''} — ${participantStatus(h,l.phase)}`;return card;}));
        $('share-room').value=joinUrl(location.href,$('backend').value,l.code);
        for(const id of ['arena-room','target-room','bots-room','apply-room'])$(id).disabled=running||!host;
        $('host-permissions').textContent=host?'You are the host. Settings changes cancel readiness.':'Only the host can change match settings.';
        $('ready-room').disabled=running;$('ready-room').textContent=l.humans.find(h=>h.id===room.participant)?.ready?'NOT READY / CANCEL':'READY';
        if(!host||!document.activeElement?.closest('#room-settings')){$('arena-room').value=l.settings.arena;$('target-room').value=l.settings.wins_target;$('bots-room').max=Math.min(3,4-l.humans.length);$('bots-room').value=l.settings.bots;}
        if(!running)show('lobby');else if(screen!=='match')show('playing');
        notice(room.notice||room.status);
      } else {
        if(pending?.kind==='connect'&&!/Connecting|Reconnecting|Retrying|Waiting for seat/.test(room.status)){pending=null;setBusy(false);}
        notice(room.notice||room.status);show('setup',false);
      }
    }
    if(pending?.kind==='command'&&Date.now()-pending.at>5000){pending=null;notice('No room update yet. Check the connection before trying again.');}
    const current=room.lobby, running=current?.phase==='Running', host=current?.host===room.participant;
    $('ready-room').disabled=!!pending||!current||running;
    $('apply-room').disabled=!!pending||!current||running||!host;
    $('leave-room').disabled=!!pending;$('leave-match').disabled=!!pending;
    let hud;try{hud=JSON.parse(env.ryggattackHud??'null');}catch{/* no valid presentation yet */}
    if(hud?.lobby?.phase==='Running'){
      if(hud.menu!==lastHudMenu){lastHudMenu=hud.menu;show(hud.menu?'match':'playing');}
    }else lastHudMenu=false;
    $('open-match-menu').hidden=screen!=='playing';
  },100);
  return {get screen(){return screen;},destroy(){env.clearInterval(timer);delete env.ryggattackNavigate;}};
}
