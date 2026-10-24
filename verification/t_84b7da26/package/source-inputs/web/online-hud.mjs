// Presentation only: timers and results come from the authoritative frame.
/**
 * Bridge contract (plain parsed JSON; never a ServerMessage envelope):
 * {
 *   lobby: null | {id:number|string, phase:'Lobby'|'Running'|{Countdown:{ticks_left:number}}},
 *   frame: null | {
 *     tick:number, simulation_tick:number, names:(string|null)[4], wins:number[4],
 *     winner:null|number, round_finished:boolean, transition:number,
 *     carts:Array<{id:number,active:boolean,alive:boolean,speed_seconds:number,all_hits_seconds:number}>
 *   },
 *   status:string, notice:string, seat:null|number, menu:boolean, rtt_ms:null|number
 * }
 * Seat/cart/winner indices are zero-based (0..3); frame.names is authoritative.
 * Blank names fall back to Bot N. Human bot takeover retains the human's name.
 * Extra fields are ignored, not copied. Do not include credentials in free-text
 * status/notice: those two public display strings are intentionally shown as-is.
 * Model: {players:[{seat,name,score,out,you,buffs:[{kind,label,seconds}]}],
 *   outcome:null|{kind:'match'|'round',seat,name,text}|{kind:'draw',text},
 *   connection,notice,menu,diagnostics:{tick,simulation_tick,rtt_ms}}.
 * Timers are snapshot values, not client clocks. Round results require
 * round_finished plus exactly one active survivor (win) or none (draw).
 * Multiple survivors have no inferred result; scores never decide a winner.
 *
 * Integration MUST clear frame on leave/reset/room change/new-match transition,
 * and publish lobby/seat/frame atomically. Protocol 3 frames have no room or
 * match identity, so a pure function cannot detect a mislabeled old frame.
 * The mount additionally quarantines identical prior frame presentations across
 * observed boundaries, including reparsed JSON. It cannot identify unseen
 * boundaries or independently establish the room of a changed stale frame.
 * No automatic bootstrap or replacement of the existing canvas HUD is performed.
 */
const validSeat = seat => Number.isInteger(seat) && seat >= 0 && seat < 4;
const positive = value => Number.isFinite(value) && value > 0;
export function buildOnlineHudModel(input = {}) {
  const frame = input.lobby?.phase === 'Running' && validSeat(input.seat) ? input.frame : null;
  const players = (frame?.carts ?? []).filter(cart => cart.active && validSeat(cart.id))
    .map(cart => ({seat:cart.id, name:frame.names?.[cart.id]?.trim() || `Bot ${cart.id + 1}`,
      score:frame.wins?.[cart.id] ?? 0, out:!cart.alive, you:cart.id === input.seat,
      buffs:[['speed','Speed',cart.speed_seconds],['all-hits','All-hits',cart.all_hits_seconds]]
        .filter(([, , seconds]) => positive(seconds)).map(([kind,label,seconds]) => ({kind,label,seconds}))}))
    .sort((a,b) => a.seat - b.seat);
  let outcome = null;
  const matchWinner = players.find(player => player.seat === frame?.winner);
  const survivors = players.filter(player => !player.out);
  const winner = matchWinner || (frame?.round_finished && survivors.length === 1 ? survivors[0] : null);
  if (winner) {
    const kind = matchWinner ? 'match' : 'round';
    outcome = {kind,seat:winner.seat,name:winner.name,text:`${winner.name} wins the ${kind}`};
  } else if (frame?.round_finished && players.length && !survivors.length) {
    outcome = {kind:'draw',text:'Round draw'};
  }
  return {players, outcome,
    connection:typeof input.status === 'string' ? input.status : '',
    notice:typeof input.notice === 'string' ? input.notice : '', menu:input.menu === true,
    diagnostics:{tick:Number.isFinite(frame?.tick) ? frame.tick : null,
      simulation_tick:Number.isFinite(frame?.simulation_tick) ? frame.simulation_tick : null,
      rtt_ms:Number.isFinite(input.rtt_ms) && input.rtt_ms >= 0 ? input.rtt_ms : null}};
}

const STYLE = `
.rygg-online-hud{position:absolute;inset:8px 8px auto;z-index:4;color:#f3f7fb;font:13px/1.3 system-ui,sans-serif;pointer-events:none;max-width:850px;margin:auto}
.rygg-online-hud [hidden]{display:none!important}
.rygg-online-hud .oh-connection,.rygg-online-hud .oh-notice,.rygg-online-hud .oh-outcome,.rygg-online-hud .oh-menu{width:fit-content;max-width:100%;box-sizing:border-box;margin:0 0 4px;padding:3px 7px;border-radius:5px;background:#10212ee8;overflow-wrap:anywhere}
.rygg-online-hud .oh-notice{border-left:3px solid #ffbf69}
.rygg-online-hud .oh-outcome{font-weight:700;border-left:3px solid #78e7ba}
.rygg-online-hud .oh-players{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:4px;list-style:none;padding:0;margin:0}
.rygg-online-hud .oh-player{min-width:0;padding:4px 7px;border-radius:5px;background:#10212ee8}
.rygg-online-hud .oh-name{display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-weight:650}
.rygg-online-hud .oh-state{display:block;font-size:12px;overflow-wrap:anywhere}
.rygg-online-hud details{margin-top:4px;width:fit-content;max-width:100%;font-size:11px}
.rygg-online-hud summary{cursor:pointer;pointer-events:auto;background:#10212ee8;border-radius:4px;padding:2px 6px}
.rygg-online-hud details[open]{pointer-events:auto;background:#10212ef5;border-radius:4px}
.rygg-online-hud pre{margin:0;padding:5px;white-space:pre-wrap}
@media(max-width:600px){.rygg-online-hud{inset:4px 4px auto;font-size:12px}.rygg-online-hud .oh-players{grid-template-columns:repeat(2,minmax(0,1fr))}.rygg-online-hud .oh-player{padding:3px 5px}}
`;

/**
 * Explicit opt-in only; no polling, globals, room commands, focus or input handlers.
 * Mount into a position:relative game wrapper, NOT the room-controls container.
 * Returns {element, update(snapshot): model, destroy()}.
 */
export function mountOnlineHud(container) {
  const doc = container.ownerDocument;
  const make = (tag, className, text = '') => {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    node.textContent = text;
    return node;
  };
  const element = make('section','rygg-online-hud');
  element.setAttribute('aria-label','Online game status');
  const style = make('style','',STYLE);
  const connection = make('p','oh-connection');
  connection.setAttribute('role','status');
  const notice = make('p','oh-notice');
  notice.setAttribute('role','alert');
  const outcome = make('p','oh-outcome');
  outcome.setAttribute('aria-live','polite');
  const menu = make('p','oh-menu');
  const players = make('ul','oh-players');
  players.setAttribute('aria-label','Active seats');
  const details = make('details','oh-diagnostics');
  const summary = make('summary','','Diagnostics');
  const diagnostics = make('pre');
  details.append(summary,diagnostics);
  element.append(style,connection,notice,outcome,menu,players,details);
  container.append(element);
  let destroyed = false;
  let previousRoom = null, lastFrameKey = null, blockedFrameKey = null;
  const put = (node,text) => { node.textContent = text; node.hidden = !text; };
  function update(input = {}) {
    let model = buildOnlineHudModel(input);
    if (destroyed) return model;
    const room = input.lobby?.id ?? null;
    const eligible = input.lobby?.phase === 'Running' && validSeat(input.seat) && !!input.frame;
    // Compare presentation-only values, not object identity: bridges may parse JSON
    // on every update. Never retain raw frames or private reconnect metadata.
    const frameKey = eligible ? JSON.stringify({players:model.players,outcome:model.outcome,
      tick:model.diagnostics.tick,simulation_tick:model.diagnostics.simulation_tick}) : null;
    if (!eligible || (previousRoom !== null && previousRoom !== room)) blockedFrameKey = lastFrameKey;
    previousRoom = room;
    if (eligible && frameKey === blockedFrameKey) model = buildOnlineHudModel({...input,frame:null});
    else if (eligible) lastFrameKey = frameKey;
    put(connection,model.connection);
    put(notice,model.notice);
    put(outcome,model.outcome?.text ?? '');
    put(menu,model.menu ? 'Input menu open · server keeps running' : '');
    players.replaceChildren(...model.players.map(player => {
      const row = make('li','oh-player');
      const name = make('span','oh-name',`${player.name}${player.you ? ' (you)' : ''}`);
      const state = [`${player.score} wins`,player.out ? 'OUT' : 'IN',
        ...player.buffs.map(buff => `${buff.label} ${(Math.ceil(buff.seconds * 10) / 10).toFixed(1)}s`)];
      row.append(name,make('span','oh-state',state.join(' · ')));
      return row;
    }));
    players.hidden = !model.players.length;
    const d = model.diagnostics;
    diagnostics.textContent = `Tick: ${d.tick ?? '—'}\nSimulation: ${d.simulation_tick ?? '—'}\nAck RTT: ${d.rtt_ms === null ? '—' : `${Math.round(d.rtt_ms)} ms`}`;
    return model;
  }
  update();
  return {element,update,destroy() { destroyed = true; element.remove(); }};
}
