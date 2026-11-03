# Offline HUD layout correction (PR 24)

The offline HUD used one absolutely positioned Bevy text node for scores, every
player's effects, match rules, power-up legend and restart instructions. On a
short landscape phone this covered the rails; on narrow Safari it could also
wrap into the arena. This was a blocking layout defect, not just clipped text.

## Layout contract

- Active offline HUD: win target; at most four player rows with wins, IN / OUT,
  and live `SPD10s` / `HIT10s` timers; one short round / match result.
- At most six lines, each at most 28 ASCII characters for legal game settings.
  The native ECS regression covers four seats, coexisting buffs, eliminated
  seats, ongoing play, round win, draw and match win.
- The browser renders the **actual ECS HUD text**, not a duplicate score model,
  in an accessible 124px band (six 18px lines plus 16px padding). There is no
  active-HUD scrolling, clipping, or rule/legend prose.
- The actual Bevy canvas is resized below that band and above an 80px control
  strip. Existing responsive camera projection then fits the arena to the
  remaining canvas on resize / rotation. Safe-area insets apply to all three
  rectangles. Online HUD and online canvas layout are unchanged.
- Rules, power-up meanings and touch / keyboard / gamepad instructions are in
  the paused-only help dialog. Only this inactive-game explanation may scroll.
  Resume / return controls precede prose, including in short landscape.
  Keyboard focus stays within its two actions and Escape resumes.
- Native pause help retains the existing Bevy menu and keyboard / gamepad path.

## Verification

`node --test scripts/touch-controls.test.mjs` covers the bridge and pause states.
`cargo test --locked --lib ui::tests` covers the source-real HUD content.
`node scripts/offline-hud-smoke.mjs` starts the real WASM game with three bots,
plays via touch, captures 375x740, 740x375, 320x640 and rotated-back viewports,
checks actual Bevy backing-size reflow and nonintersecting rectangles, and
captures pause help at phone and short-landscape sizes. This is not a DOM-only
fixture or a claim of physical iPhone Safari verification.

Fresh compiler receipts, artifact/source hashes, timings, screenshots and
immutable red runs are retained privately under
`/home/hermes/.hermes/cache/scratch/mobile-hud-fix/`. Earlier accepted evidence
under `docs/mobile-touch*` is unchanged. WebKit was not installed on the host;
no large browser installation is required for this fix. Actual iPhone Safari,
real notches, browser chrome and OS focus behavior remain a manual check.
