# Clone Feature Tracker

## Context Sources
- README and docs
- TODO/FIXME markers in code
- Test and build failures
- Gaps found during codebase exploration

## Candidate Features To Do

- [ ] P0 (Selected): Ship privacy exclusions from local config: `deny.apps` + browser private-window skip (Chromium adapters first), enforced pre-capture with `CaptureSkipped` engine events and context-log entries that never include window titles/URLs.
- [ ] P0 (Selected): Rename `readme.md` -> `README.md` so GitHub renders the project correctly; keep all behavior docs aligned with code.
- [ ] P1 (Selected): Harden repo hygiene: expand `.gitignore` to avoid accidental commits of runtime artifacts (captures, screenshots), and remove stray local scratch files.
- [ ] P1 (Selected): Surface privacy status + config path actions in the menu bar (open/reload) and in CLI help (`--privacy-config`).
- [ ] P1: Implement runtime idle/screen-lock auto-pause with explicit `AutoPaused/AutoResumed` engine events (lock/sleep first; static-screen detector behind a flag).
- [ ] P1: Decouple analysis from capture path with bounded async queue + retry drain semantics (pre-req for crash recovery).
- [ ] P1: Add launch-agent diagnostics command/menu action to self-heal startup-on-login failures.
- [ ] P1: Add queue/latency/session telemetry counters in CLI + menu bar status.
- [ ] P2: Add golden-format tests for `context.md` entries across failure and multiline-summary cases.
- [ ] P2: Add a minimal end-to-end smoke script (`scripts/smoke.sh`) that validates a no-analyze run path and logs expected permission gating.

## Implemented

- 2026-02-08: OpenAI analyzer now enforces 30s request timeout, bounded retry/backoff on transient 429/5xx/connect/timeout failures, and malformed success-payload fallback summaries (src/analysis.rs, readme.md, features.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`). Prevents API instability from stalling capture loops.
- 2026-02-08: Added analyzer fault-injection tests covering transient retry recovery, non-retryable 400 handling, explicit timeout behavior, and malformed-payload fallback parsing (src/analysis.rs, `cargo test analysis::tests::retries_transient_http_error_and_succeeds -- --exact`, `cargo test`).
- 2026-02-08: Added engine fault-injection tests for screenshot provider failures and context-log write failures; both now verify deterministic failure accounting without session crash (src/engine.rs, `cargo test`).
- 2026-02-08: Added parser regression tests for human-readable byte-size guardrail input (`1.5GB`, underscores, invalid units) and fixed strict clippy findings across CLI/menu/storage/permissions/context modules (src/main.rs, src/bin/menubar.rs, src/storage.rs, src/permissions.rs, src/context_log.rs, `cargo clippy --all-targets --all-features -- -D warnings`).
- 2026-02-08: Added GitHub Actions CI workflow for `cargo fmt --check`, strict clippy, and tests on push/PR (`.github/workflows/ci.yml`).
- 2026-02-08: Added implementation design docs for idle/screen-lock auto-pause and privacy exclusions to guide next milestone execution (docs/idle-autopause-design.md, docs/privacy-controls-design.md).
- 2026-02-08: Screencapture watchdog aborts hung captures after 10s with actionable guidance (src/screenshot.rs, readme.md, features.md, cargo test). Prevents macOS permission prompts from freezing the entire session loop.
- 2026-02-08: Disk guard cleanup events now raise CLI + tray notifications showing deleted file count and freed/remaining space (src/engine.rs, src/main.rs, src/bin/menubar.rs, readme.md, features.md, cargo test). Gives operators immediate visibility whenever automatic pruning removes captures.
- 2026-02-08: Disk guard now reclaims the oldest captures before bailing (src/storage.rs, src/engine.rs, readme.md, features.md, cargo test). Keeps production machines running by freeing space automatically when the 1 GiB threshold trips.
- 2026-02-08: Menu bar icon reflects capture state (src/bin/menubar.rs, readme.md). Adds instant visual cue for running/paused/error.
- 2026-02-08: Finder shortcuts for log and captures (src/bin/menubar.rs, readme.md). Restores rapid inspection path when debugging AI output.
- 2026-02-08: Menu exposes file-aware \"Open latest capture\" quick link (src/bin/menubar.rs, readme.md). Enables one-click auditing/deletion of the newest screenshot.
- 2026-02-08: Screen recording permission health check blocks sessions until macOS access is granted (src/main.rs, src/bin/menubar.rs, src/permissions.rs, readme.md). Prevents silent zero-capture runs and deep-links users to System Settings.
- 2026-02-08: Menu surfaces live Screen Recording status plus Recheck/Open Settings actions (src/bin/menubar.rs, readme.md). Gives users an always-on diagnostic panel when macOS revokes access mid-session.
- 2026-02-08: Disk space guard halts capture sessions when free space dips below configurable threshold (src/storage.rs, src/engine.rs, src/main.rs, src/bin/menubar.rs, readme.md, cargo test). Prevents runaway storage exhaustion on production laptops.
- 2026-02-08: Permission watchdog auto-pauses sessions when macOS revokes Screen Recording mid-run and resumes when access returns (src/permission_watch.rs, src/main.rs, src/bin/menubar.rs, readme.md, features.md, cargo test). Prevents silent capture failures and keeps context logs accurate even when privacy prompts pop mid-session.

## Insights

- A local mock HTTP server test harness gives deterministic coverage for API retry/timeout semantics without requiring live OpenAI credentials in CI.
- Real CLI smoke captures are currently gated by macOS Screen Recording entitlement; this should be documented as an expected precondition for manual verification.
- Strict clippy (`-D warnings`) surfaced multiple unit-return and collapsible-if issues that are easy to miss in fast iteration; keeping this gate in CI materially improves maintainability.
- Users rely on the tray icon more than menu text when screens are crowded; color coding makes the current session state legible at a glance and lowers anxiety.
- Rapid access to captures/context is essential when auditing AI summaries or deleting sensitive shots; surfacing these actions from the tray avoids Finder spelunking.
- Showing the newest capture filename directly in the menu reduces guesswork when multiple sessions run per day and encourages immediate cleanup of sensitive frames.
- Apple now re-prompts for Screen Recording access on a roughly monthly cadence, so surfacing a live status plus a one-click recheck keeps trust high when captures suddenly stall.
- Automatic capture pruning keeps sessions alive without user action, but we still need to surface a heads-up (menu toast + Finder link) when files are removed so advanced users trust the cleanup.
- Screencapture can hang silently when macOS loses permission mid-run; adding a watchdog makes failures obvious, but we still need proactive permission flip detection so we can auto-stop before the timeout hits.
- Permission flips now auto-pause/resume sessions, and users immediately see status copy plus icon changes, which keeps trust high; next step is tying these events into analytics so we can measure how often Apple revokes access.
- Silent disk cleanup eroded trust; now that CLI/menu surfaces reclaimed file counts with remaining headroom, operators immediately understand what changed and can archive sensitive captures before they vanish again.

## Notes
- This file is maintained by the autonomous clone loop.

### Verification Evidence (2026-02-08)
- PASS: `cargo test`
- PASS: `cargo clippy --all-targets --all-features -- -D warnings`
- PASS: `cargo run --bin photographic-memory -- plan`
- PASS: `cargo test analysis::tests::retries_transient_http_error_and_succeeds -- --exact`
- PASS: GitHub Actions CI run `21806764636` (`https://github.com/sarveshkapre/photographic-memory/actions/runs/21806764636`)
- BLOCKED (permission): `cargo run --bin photographic-memory -- immediate --no-analyze --min-free-bytes 0 --output-dir /tmp/pm-smoke-34Pj4v/captures --context /tmp/pm-smoke-34Pj4v/context.md --filename-prefix smoke` -> `Screen Recording permission is denied`

### Auto-discovered Open Checklist Items (2026-02-08)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] App/website exclusion list + incognito/private-window auto-exclusion
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] One-click pause + visible "capturing now" indicator in menu bar
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Auto-pause on sleep/idle/static-screen and maintenance when idle
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Startup on login + self-heal background agent diagnostics
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Pin screenshot as always-on-top floating reference card
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] OCR quick-copy flow (single hotkey, auto-copy recognized text)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] QR detection in screenshot regions
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Built-in annotation toolkit (arrow, highlight, text, blur/pixelate)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Sensitive-data smart redaction assistant (emails, phones, cards, URLs)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Scrolling capture with fallback path and clear failure guidance
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] URL scheme/deep-link API for scripted actions
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Post-capture action pipeline (CLI hooks)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Reusable effect presets auto-applied after capture
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] S3-compatible upload targets + short-link return
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Condensed timeline view (collapse long no-activity gaps)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Search modes: OCR text, visual tags, app filter, time filter
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] Session metrics dashboard (captures/hour, GB/month, queue lag, failures)
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] 1. Exclusion list + private window exclusion
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] 2. Auto-pause on idle/static + storage guardrails
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] 3. OCR quick-copy + smart redaction
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] 4. URL scheme + action pipeline hooks
- /Users/sarvesh/code/photographic-memory/todo.md:- [ ] 5. Condensed timeline with app/time filters
- 2026-02-08T08:12:07Z: checkpoint commit for pass 1/1 (no meaningful code delta found).
