// Real replacement UI selectors shared by graphical acceptance harnesses.
export async function openRoomMenu(page) {
  if(await page.locator('#play-online').isVisible())await page.locator('#play-online').click();
  if(await page.locator('#online-setup').isVisible()) {
    const advanced=page.locator('#online-setup details');
    if(!await advanced.evaluate(e=>e.open))await advanced.locator('summary').click();
  } else if(await page.locator('#open-match-menu').isVisible())await page.locator('#open-match-menu').click();
}
export async function closeRoomMenu(page) {
  if(await page.locator('#resume-match').isVisible())await page.locator('#resume-match').click();
  // Lobby is intentionally a full menu, not a collapsible gameplay overlay.
}
export async function admitInvitation(page) {
  await page.waitForFunction(()=>typeof globalThis.ryggattackBrowserMenu==='boolean');
  if(await page.evaluate(()=>!!globalThis.ryggattackLaunch))return; // saved seat
  await openRoomMenu(page);
  const join=!!await page.locator('#room-code').inputValue();
  await page.locator(join?'#join-room':'#create-room').click();
}
