// HTML controls dispatch through the same Rust adapter as native keys.
export function joinUrl(base, address, code) {
  const url=new URL(base); url.searchParams.set('online',address); url.searchParams.set('room',code); url.searchParams.delete('name'); return url.href;
}
export function settingsCommand(arena, target, bots) {
  const wins_target=Number(target); bots=Number(bots);
  if(!['Forest','Volcano'].includes(arena)||![1,3,5,7].includes(wins_target)||!Number.isInteger(bots)||bots<0||bots>3)throw Error('Invalid settings');
  return {Configure:{arena,wins_target,bots}};
}
export function installRoomControls() {
  const $=id=>document.getElementById(id), params=new URLSearchParams(location.search);
  $('backend').value=params.get('online')??'ws://127.0.0.1:8787';
  $('player-name').value=params.get('name')??'Player';
  $('room-code').value=params.get('room')??'';
  if(params.has('room')) $('online-entry').open=true;
  const send=command=>{if(globalThis.ryggattackCommand){$('room-notice').textContent='Please wait for the previous command';return;}globalThis.ryggattackCommand=JSON.stringify(command);};
  $('create-room').onclick=()=>{$('room-code').value='';$('room-code').required=false;};
  $('join-room').onclick=()=>{$('room-code').required=true;};
  $('ready-room').onclick=()=>{const s=JSON.parse(globalThis.ryggattackRoom??'{}');send({Ready:!s.lobby?.humans.find(h=>h.id===s.participant)?.ready});};
  $('apply-room').onclick=()=>{try{send(settingsCommand($('arena-room').value,$('target-room').value,$('bots-room').value));}catch(error){$('room-notice').textContent=error.message;}};
  $('leave-room').onclick=()=>send('Leave');
  $('copy-room').onclick=async()=>{try{await navigator.clipboard.writeText($('share-room').value);$('room-notice').textContent='Link copied';}catch{$('share-room').select();$('room-notice').textContent='Select and copy this link';}};
  let previous='';
  setInterval(()=>{
    const text=globalThis.ryggattackRoom;if(!text||text===previous)return;previous=text;
    const s=JSON.parse(text), l=s.lobby; $('room-controls').hidden=!l;
    $('room-notice').textContent=s.notice||s.status;
    if(!l)return;
    $('room-info').textContent=`Room ${l.code} · ${typeof l.phase==='string'?l.phase:`Starting in ${(l.phase.Countdown.ticks_left/64).toFixed(1)}s`}\n`+l.humans.map(h=>`${h.name}${h.id===l.host?' (host)':''}: ${h.ready?'ready':'not ready'}`).join('\n');
    $('share-room').value=joinUrl(location.href,$('backend').value,l.code);
    const running=l.phase==='Running', host=l.host===s.participant;
    for(const id of ['arena-room','target-room','bots-room','apply-room'])$(id).disabled=running||!host;
    $('ready-room').disabled=running;
    $('ready-room').textContent=l.humans.find(h=>h.id===s.participant)?.ready?'Unready / cancel':'Ready';
    // Do not overwrite edits while the host is choosing settings.
    if(!host||!document.activeElement?.closest('#room-settings')){
      $('arena-room').value=l.settings.arena;$('target-room').value=l.settings.wins_target;
      $('bots-room').max=4-l.humans.length;$('bots-room').value=l.settings.bots;
    }
  },100);
}
