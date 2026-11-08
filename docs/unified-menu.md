# Integrated local / online menus

The browser starts with PLAY LOCAL and PLAY ONLINE in the existing navy/gold/blue menu language. Local opens the existing Bevy cart setup; online opens a full menu with player name, explicit create/join buttons, a labeled room code and an Advanced backend field. The production default is same-origin `wss://<host>/ws`; HTTP loopback development retains port 8787.

## Ownership and navigation

Local simulation and authoritative online presentation remain separate Bevy applications. The browser menu is a shared presentation shell, not another networking client. Selecting local uses the existing `AppState::Lobby`. Selecting online or typing a code never connects. An explicit valid create/join activation stores a one-shot launch intent in sessionStorage and navigates to the selected adapter, preserving the quality pin. Reload consumes the intent once. Otherwise only a structurally valid endpoint/room-scoped saved claim permits automatic reconnect; fresh invitations open setup without admission. Missing sessionStorage produces visible feedback rather than silently risking duplicate admission.

The lobby derives seats, host, settings, readiness and countdown exclusively from `ryggattackRoom`, published by the existing Rust NetworkClient. Player names are text, not HTML. All humans still ready/unready through the existing protocol; there is no new Start or ReturnToLobby command. The server returns finished matches to their lobby. Leaving and returning after a match are explicitly distinct.

LEAVE ROOM sends the existing Leave command and waits for the Rust adapter's cleared lobby and terminal left status before presenting setup. BACK then reloads the local application with online parameters removed; this destroys online visual/input resources rather than retaining a hidden client. A cancellation after Hello or during reconnect keeps polling until validated Welcome, then sends Leave, so an admitted seat is revoked instead of abandoned as a reservation. If transport recovery fails before release can be sent, the UI explicitly says release is unconfirmed and that a disconnected reservation may last up to 60 seconds; it does not claim success. Escape in a text field only exits editing; Escape from a lobby never disconnects. A separate explicit Leave activation is required.

During Running, create/join and lobby settings are hidden. The existing authoritative HUD remains. The contextual menu offers Resume Input and Leave Room and states that the server keeps running. Opening it suppresses input, not server time. Existing F3/video settings and local cart controls remain.

## Input and layout

Bevy's existing keyboard/gamepad MenuInput collector forwards intent into visible browser controls, then clears it before the canvas menu can also consume it. DOM-focused typing does not become canvas keyboard shortcuts. Browser arrows/Tab/Enter use actual focusable controls; field Escape does not act as Back in the same event. Controller directions can step out of fields, Left/Right adjusts focused host select/number settings (Apply remains explicit), and controller Back ends editing before navigating. Idle bridge frames preserve focus. Controller Start opens the online in-match input menu; it never pauses the server. Menus restore focus on screen changes and canvas focus on gameplay/local entry.

The menu is vertically scrollable, with bounded-width fields/cards and a one-column roster on narrow viewports. The existing local cart screen scales its UI to fit narrow browser windows; this is presentation scaling, not a touch gameplay implementation. No duplicate corner room disclosure is retained.

## Desktop entry

The desktop main menu also offers PLAY LOCAL and PLAY ONLINE. Online setup has
editable player-name, backend-address, and room-code fields and explicit
create/join actions. The desktop defaults to
`wss://ryggattack.modig.online/ws`; use `ws://127.0.0.1:8787` with the local
development server. A browser and desktop player using the same endpoint and
room code occupy the same authoritative room; there is no desktop-only server
or alternate gameplay protocol.

The graphical desktop flow reuses the Rust `NetworkClient` and online snapshot
presentation. Offline simulation does not run online. Leaving releases the
seat through the existing client lifecycle, and returning to local play rebuilds
local scene presentation rather than retaining an active hidden online session.
Opening an online input menu suppresses that player's controls without pausing
the server. Controls must return to neutral before gameplay input resumes, so
Space or a controller's accept button cannot also fire when closing the menu.
Fields support Unicode text, Backspace and Ctrl+A replacement; Enter/Escape
finishes editing, and a mouse click can switch fields or activate a button.
The room lobby and online input menu include COPY INVITE LINK and COPY ROOM CODE.
Only a successful OS clipboard write produces a copied confirmation; failures
remain visible. Native builds explicitly enable Bevy's system-clipboard backend,
not its in-process fallback buffer.

PASTE CODE / INVITE and Ctrl/Cmd+V in the room-code field accept a code or a
browser invitation. Parsing validates bounded input, endpoint scheme/authority,
the exact room-code format, and unambiguous `online`/`room` parameters before
changing either connection field. Pasting never connects. Name/backend fields
also support ordinary clipboard text with their existing length limits.

`src/invitation.rs` owns the public invitation data and browser-link format,
independent of menus, clipboard, networking admission, or any Steam SDK. Only
backend and room code are encoded; unrelated base-URL parameters and fragments
are removed. `RYGGATTACK_WEB_URL` selects a custom browser frontend; it is not a
WebSocket endpoint. HTTP browser hosts are required for plain-WS backend links.
The default frontend is production, or beta for the beta backend. Future
platform invitations can adapt this data without introducing another multiplayer
protocol. Steam authentication, overlays, and friend invitations are not added.

The `--online`, `--name`, and `--room` command-line entry remains
supported independently of the graphical setup.

The browser retains its DOM form and reload-based adapter selection. Desktop
entry does not change browser invitations or scoped sessionStorage reconnect
claims. Automated native checks do not establish physical-controller or
public-deployment acceptance; record those separately when exercised.

## Verification entry points

- `cargo test --locked --lib invitation::tests -j 1 -- --test-threads=1`: browser-link roundtrips, public-field whitelist, literal code/backend preservation, and malformed/ambiguous input rejection.
- `cargo test --locked --lib online::desktop::sharing -j 1 -- --test-threads=1`: copy payloads, clipboard write failure, local frontend selection, and atomic paste behavior. OS clipboard I/O is injected at the external boundary rather than overwriting the user's clipboard during tests.
- `cargo test --locked --lib online::desktop -j 1 -- --test-threads=1`: native setup/input checks and real loopback-server create/join, host permissions, readiness, authoritative gameplay, leave and repeated local-scene restoration. These tests run the ECS presentation pipeline without a graphical window; they are not a physical-controller or browser/desktop graphical playtest.
- `node scripts/rooms-ui.test.mjs`: launch validation, invitations, private scoped resume, duplicate guard, host permissions, phase/lifecycle presentation, recovery and shared navigation with a deterministic DOM boundary fixture. This fixture does not establish transport or gameplay correctness.
- `node --test scripts/refresh-rate.test.mjs scripts/online-hud*.test.mjs`: unchanged calibration/fallback and existing HUD contracts, plus initial menu focus.
- `PLAYWRIGHT_NODE_MODULES=/absolute/path/to/playwright/package.json node scripts/unified-menu-smoke.mjs verification/unified-menu`: real local-only backend, two independent Chromium/SwiftShader participants, normal menu buttons, local entry, private room lifecycle, natural match completion, saved-seat reload, explicit Leave, local-after-online and error paths. The harness records source/artifact hashes, screenshots and exact failures; only a passed report is acceptance evidence. It never visits the public service.
- `scripts/menu-harness.mjs`: replacement selectors used by existing acceptance harnesses; Advanced is expanded only in setup and gameplay menus use Resume rather than a hidden compatibility disclosure.

Historical retained evidence describes its original artifacts and is not relabeled as proof of this menu. `verification/` remains private/ignored: reconnect credentials and session storage must never be published. Current replay results, timings, source hashes, limitations and visual screenshot review belong in the task/PR handoff.
