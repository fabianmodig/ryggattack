Evidence preparation — t_84b7da26 (not delivery)

Candidate checkout: /home/hermes/src/ryggattack-online-explosions
Branch: fix/online-explosion-scale
Base HEAD: f5ae645545babe5e4127d27c8d22ca123839ff66
Final proposed commit: NOT ESTABLISHED. Await explicit permission for local candidate/evidence commits; no push, merge or deployment is requested.
Tested state is the dirty source manifest from run106 (119 members, SHA256 d5b6ff99e2d5fff7861a067d951d646d0e5133af98b8cd6cda751b2603aebcd4) plus tracked patch a5118f24a2ffe925151573da0d5cce3cb6eace6bce84f53d74a66a4e17fa63de and recorded browser harness/source hashes. Do not call the base SHA the tested committed fix. No product changes were made by this task.

Actual new execution
  1. node verification/t_84b7da26/preflight.mjs: verified source manifest, candidate patch, native binaries and all nine bundle files. Preserved prior dist/web at dist/web-before-t_84b7da26 and deliberately copied the separately verified bundle, preserving timestamps. Zero builds.
  2. RYGGATTACK_EXPLOSION_ARTIFACT_READY=1 PLAYWRIGHT_NODE_MODULES=/home/hermes/.npm/_npx/e41f203b7505f1fb/node_modules/playwright/package.json RYGGATTACK_NATIVE_BIN_DIR=/home/hermes/src/ryggattack-online-explosions/target/debug node scripts/online-explosion-smoke.mjs --local: exit0, captured-review-required. Unique original attempt verification/online-explosion/2026-10-24T15-30-37-066Z-fe8307a389d1. Two independent actual rendered WASM humans, real UI/input, complete shared authoritative snapshots, natural primary/wall/prop-chain correlations and clean explicit Leave/browser/backend/HTTP cleanup. Served every bundle file and checked exact bytes/MIME. Original status remains unchanged.
  3. Same PLAYWRIGHT_NODE_MODULES and RYGGATTACK_NATIVE_BIN_DIR, node scripts/online-hud-smoke.mjs --local: exit0, passed. Original verification/online-hud/2026-10-24T15-33-35-458Z-901957. One actual WASM human plus bot, target-seven focus/menu input preflight and independent target-one natural-completion core, authoritative matching outcomes, fresh reset and bounded lobby clear. Real game page closes before synthetic presentation fixtures. No naturally observed buff in these HUD cases; synthetic timer fixtures do not prove natural buff collection. Explosion image B1/B2 independently show real Speed timer labels.
  4. node verification/t_84b7da26/offline.mjs: exit0, captured-review-required. Original verification/t_84b7da26/offline. One real WASM page, ordinary keyboard menu/lobby/play/pause/resume and no WebSocket/backend. Actual screenshots inspected. P1 is already OUT in gameplay captures; Space key events do NOT prove local firing. Offline lobby has existing viewport clipping at640x480; no claim of responsive-lobby perfection.
  5. node verification/t_621f9087-review116/check.mjs > verification/t_84b7da26/cached-cpu-node.log: checker exit0. Fresh execution of retained native test binary passed14 online tests,1 offline/server physical rear-contact test,4 shared rear-contact tests; fresh143 Node tests passed. This is cached-binary execution, not a new compile/full203-test rerun. Checker log's 'no browser runs' line describes that checker's scope, not the whole task. A following ps returned1 because none of the named processes existed; this is not a test failure.
  6. node verification/t_84b7da26/rear-wire.mjs: read-only shared-browser wire analysis found ZERO qualifying straight aligned rear contacts. This is a coverage gap, NOT a browser rear-contact pass. Exact0.4 leader-only displacement and no reversal are proved by the fresh paired offline/server physical CPU fixture (both front-seat choices) and shared regression suite, not by these pixels or a fabricated browser contact. A controlled legal-input browser rear-contact replay remains outstanding if literal browser coverage of that event is required.

Manual visual review (actual pixels, all18 PNGs inspected)
  Explosion A1,A2,A3: enlarged orange blasts visibly wider than cart bodies/rail spacing at differing locations/sizes; HUD correct distinct names/own-seat. B1,B2: enlarged blasts and real Speed timer visible. B3: real gameplay/HUD but NO visible blast; do not count it positive. Thus3 positive images forA and2 forB meet the harness checklist. Six distinct hashes retained with bracketing tick/phase metadata. No exact wire-tick-to-pixel synchronization or quantitative GPU bounds claim.
  HUD preflight-playing shows named Bot2 round winner and OUT/IN roster. actual-playing shows fresh zero scores, not winner pixels. Both returned-lobby images have cleared roster/outcome. fixture1024/375 are visibly labeled synthetic, escaped-name/long-notice/timer layout only, not gameplay.
  Offline menu, lobby, playing, firing, pause, resumed inspected; pause has actual PAUSED overlay and resumed has none. No private auth/claims/invitations observed in any inspected image. Visual review is not a general OCR guarantee.
  Combined visual enlargement acceptance uses the real CPU mesh vertex-bounds oracle, not wire scale alone. The old broken mesh could pass wire-transform checks.

Artifact identity and build handoff
  Verified bundle: /home/hermes/.hermes/artifacts/ryggattack/wasm-packaging-20261024/bundle
  Optimized WASM: fbe27a35a166a38a897118c580ee789063c3da4df28bf7bf1100e50430b2db52
  Generated JS: 13e7f4d5b8cee60ee75be214227c7b976a87db00d2570e2cca8d49efabd0243a
  Native backend: 90af59595497af21b5c0ba6e738a290183214760be298ecefb2ff212fc1a8dea
  Native headless: 7a3d72e0ac91d84292d8ecc1bd11a69badaf50fd4f75d50936c3b1835212c846
  Input WASM: 90b7c369b59b517f88239f38ab77d5dbe0a3c9f7e550a25e549a54f2a7a0b625
  Retained run106 full Rust203 passed231.934s; clippy3.530s exit0 with4 warnings; cached native build0.446s; headless1.444s; authoring Node157 passes1.194s; other authoring0.346s. Exact prior CPU argv were not stored; do not invent flags.
  Retained real mesh RED exit10116.684s; initial unavailable-Cargo attempt exit1270.000s; interrupted WASM wrapper exit143240.506s (compiler3m55s), not successful packaging.
  Successful separate packaging: cached frozen release compiler0.675s, wasm-bindgen9.641s, wasm-opt223.154s. Complete argv and hashes retained in parent sanitized derivative. No build was run by this task. No compiler, linker or optimizer or unrelated SwiftShader game was running during any browser gate; gates ran sequentially after exact bundle verification, with process checks before/after.

Historical/private preservation, NOT new-head proof
  HUD runtime11b9afce48aec0d9ff7ea878003c4ca5365e6f6b and chart/main/Fluxf5ae645545babe5e4127d27c8d22ca123839ff66 remain HISTORICAL.
  Restricted durable original /home/hermes/.hermes/artifacts/ryggattack/rollout-2026-10-23/ryggattack-hud-11b9afc-verified.json SHA2564ddd0c29cdfbccc4857bfa9e9a1ed2876c98b6e1894f9bc9877c5fb1e3183453. Original scratch source was lost historically; recovery receipt and restricted durable original remain, not a recreated scratch file.
  Baseline /home/hermes/.hermes/artifacts/ryggattack/baseline-t_7969543b/BASELINE.txt SHA256b184cf14cf84f693a04d92d55b2437d6fe65421bbc10594a7c214253eef39db5.
  Historical originals and RED/GREEN/privacy/review records: baseline restricted directory and preserved-sha256.txt (copied receipt in package). All original failures remain unchanged. Failed privacy marker SHA25635b1845718e01f1073eeb455f3b248efdd7761655ebf3c8d37a583c823d4aa43 remains failed; private purged artifacts are not reconstructed.
  Only allowlisted sanitized derivatives enter the proposed index. Historical originals remain private, not certified globally safe. Broad ignored verification inventory includes TLS material; excluded without opening or hashing TLS files. No blanket force-add.

Review reproduction (read-only, no builds)
  cd /home/hermes/src/ryggattack-online-explosions
  node verification/t_84b7da26/package/verify.mjs --checkout
  Inspect MANIFEST.json and RECONCILIATION.json; compare each listed member's disk and git show :path SHA256. Self-exclusions are explicit; the audit report cannot recursively hash itself. Read source-inputs.sha256, retained CPU/WASM metadata and full compressed sanitized traces; inspect actual PNGs. No public room or production restart needed.
  Review package integrity independently of current checkout by omitting --checkout. After candidate commits are authorized, rerun --checkout plus index reconciliation and bind exact final SHA in a separate receipt (no runtime edits). A later product change invalidates affected checks; rerun them before claiming coverage. Evidence-only commits do not rebuild or relabel artifact source provenance.

Outstanding gates and limitations
  No final proposed commit SHA yet; no independent exact-head approval, remote CI, publication, merge or delivery claimed. Controlled browser rear-contact event is not established by current captures; CPU exact oracle passes. Anonymous bearer theft remains accepted: possession of a currently valid reconnect bearer can replace an owner; no theft resistance claimed. Native manual graphics deferred separately, not a web blocker. Historical failures, incomplete command provenance, clippy warnings and offline viewport clipping are retained rather than hidden.
