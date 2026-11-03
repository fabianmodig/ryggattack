# PR 24 pause W/S repair

The DOM pause dialog introduced in `49cba9281bf7a06c8fb221a7619a244009e702cd`
focuses its Resume button, so the existing Rust keyboard adapter deliberately
ignores keyboard input while that button has focus. The DOM handler retained
arrows and Tab, but omitted the existing physical W/S navigation bindings.

The runtime repair changes only that handler's navigation predicate: `KeyW` /
`KeyS`, plus case-insensitive `w` / `s` key fallbacks, use the existing two-button
focus toggle and DOM-to-ECS focus publication. Arrows, Tab / Shift+Tab, native
Enter / Space button activation, Escape, and selection-to-DOM gamepad mirroring
are unchanged. No Rust, layout, build, server, or cluster changes are included.

## Regressions

- `node --test scripts/touch-controls.test.mjs`: physical codes on a non-Latin
  layout, lowercase/uppercase key fallbacks, arrows, Tab/Shift+Tab, publication,
  native-activation noninterference, and selection-to-DOM mirroring.
- `node scripts/pause-keyboard-smoke.mjs`: real rendered WASM game, trusted
  keyboard W/S, arrows, Tab/Shift+Tab, actual ECS selection publication, Enter /
  Space resume, Escape from Return to Menu, and S + Enter returning to MainMenu.
  `PAUSE_BUNDLE` selects the artifact root; `PAUSE_EVIDENCE` selects a new output
  directory. Playwright is supplied through `PLAYWRIGHT_NODE_MODULES`.
- The browser verifier waits for the DOM focus mailbox to be consumed and the
  presentation poll to settle before the next key. Earlier verifier versions
  and failures remain archived: v1 incorrectly expected Lobby rather than
  MainMenu after Return to Menu; v2 observed a rapid-key/presentation race.
  Neither failure is relabeled as passing. Same-frame burst behavior is not
  established by the settled-input replay.

## Evidence and limits

`docs/mobile-pause-ws-repair.json` binds source, verifier, full bundle, historical
compiler receipts and replay results. Private raw evidence is retained at
`/home/hermes/.hermes/cache/scratch/hud-ws-repair/`; original HUD evidence under
`mobile-hud-fix/` is unchanged. The corrected `candidate-bundle/` has **mixed
provenance**: the exact retained fresh WASM/bindgen and all other bundle files,
with only `touch-controls.mjs` restaged. This is not a new WASM compilation.

Physical Safari/iPhone and physical gamepad operation remain unverified. The
unit regression covers the unchanged gamepad-selection publication mirror;
the browser replay exercises real keyboard-to-DOM-to-ECS behavior, not a
physical gamepad. Native Cargo/clippy, optional wasm-opt, full layout/online
replays, CI, publication, and independent re-review were not rerun here.
No push, merge or deployment is authorized by this repair.
