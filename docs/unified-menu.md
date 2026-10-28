# Integrated local / online browser menu

The browser starts with PLAY LOCAL and PLAY ONLINE in the existing navy/gold/blue menu language. Local opens the existing Bevy cart setup; online opens a full menu with player name, explicit create/join buttons, a labeled room code and an Advanced backend field. The production default is same-origin `wss://<host>/ws`; HTTP loopback development retains port 8787.

## Ownership and navigation

Local simulation and authoritative online presentation remain separate Bevy applications. The browser menu is a shared presentation shell, not another networking client. Selecting local uses the existing `AppState::Lobby`. Selecting online or typing a code never connects. An explicit valid create/join activation stores a one-shot launch intent in sessionStorage and navigates to the selected adapter, preserving the quality pin. Reload consumes the intent once. Otherwise only a structurally valid endpoint/room-scoped saved claim permits automatic reconnect; fresh invitations open setup without admission. Missing sessionStorage produces visible feedback rather than silently risking duplicate admission.

The lobby derives seats, host, settings, readiness and countdown exclusively from `ryggattackRoom`, published by the existing Rust NetworkClient. Player names are text, not HTML. All humans still ready/unready through the existing protocol; there is no new Start or ReturnToLobby command. The server returns finished matches to their lobby. Leaving and returning after a match are explicitly distinct.

LEAVE ROOM sends the existing Leave command and waits for the Rust adapter's cleared lobby and terminal left status before presenting setup. BACK then reloads the local application with online parameters removed; this destroys online visual/input resources rather than retaining a hidden client. Leaving during connection uses the same transport-close path. Escape in a text field only exits editing; Escape from a lobby never disconnects. A separate explicit Leave activation is required.

During Running, create/join and lobby settings are hidden. The existing authoritative HUD remains. The contextual menu offers Resume Input and Leave Room and states that the server keeps running. Opening it suppresses input, not server time. Existing F3/video settings and local cart controls remain.

## Input and layout

Bevy's existing keyboard/gamepad MenuInput collector forwards intent into visible browser controls, then clears it before the canvas menu can also consume it. DOM-focused typing does not become canvas keyboard shortcuts. Browser arrows/Tab/Enter use actual focusable controls; field Escape does not act as Back in the same event. Controller directions can step out of fields, Left/Right adjusts focused host select/number settings (Apply remains explicit), and controller Back ends editing before navigating. Idle bridge frames preserve focus. Controller Start opens the online in-match input menu; it never pauses the server. Menus restore focus on screen changes and canvas focus on gameplay/local entry.

The menu is vertically scrollable, with bounded-width fields/cards and a one-column roster on narrow viewports. The existing local cart screen scales its UI to fit narrow browser windows; this is presentation scaling, not a touch gameplay implementation. No duplicate corner room disclosure is retained.

Native local menu/shared adapter checks remain supported; native online launch still uses `--online` and its existing keyboard controls. This is a web-first integration, not a claim of a new native text-entry menu. Native graphical interaction and physical-controller testing remain deferred/unverified unless separately recorded.

## Verification entry points

- `node scripts/rooms-ui.test.mjs`: launch validation, invitations, private scoped resume, duplicate guard, host permissions, phase/lifecycle presentation, recovery and shared navigation with a deterministic DOM boundary fixture. This fixture does not establish transport or gameplay correctness.
- `node --test scripts/refresh-rate.test.mjs scripts/online-hud*.test.mjs`: unchanged calibration/fallback and existing HUD contracts, plus initial menu focus.
- `PLAYWRIGHT_NODE_MODULES=/absolute/path/to/playwright/package.json node scripts/unified-menu-smoke.mjs verification/unified-menu`: real local-only backend, two independent Chromium/SwiftShader participants, normal menu buttons, local entry, private room lifecycle, natural match completion, saved-seat reload, explicit Leave, local-after-online and error paths. The harness records source/artifact hashes, screenshots and exact failures; only a passed report is acceptance evidence. It never visits the public service.
- `scripts/menu-harness.mjs`: replacement selectors used by existing acceptance harnesses; Advanced is expanded only in setup and gameplay menus use Resume rather than a hidden compatibility disclosure.

Historical retained evidence describes its original artifacts and is not relabeled as proof of this menu. `verification/` remains private/ignored: reconnect credentials and session storage must never be published. Current replay results, timings, source hashes, limitations and visual screenshot review belong in the task/PR handoff.
