// Standalone/closure-free so Playwright can serialize this into the real page.
// Missing initial publication is not an error or a pass; malformed JSON still fails.
export function roomStatusHas(fragment) {
  const text=globalThis.ryggattackRoom;
  if(text===undefined||text===null)return false;
  const state=JSON.parse(text);
  return typeof state.status==='string'&&state.status.includes(fragment);
}
