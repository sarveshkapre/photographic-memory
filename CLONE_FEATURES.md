# Clone Feature Tracker

## Context Sources
- README and docs
- TODO/FIXME markers in code
- Test and build failures
- Gaps found during codebase exploration

## Candidate Features To Do

- [ ] P1: Add optional static-screen auto-pause (hash sampling) behind an explicit opt-in flag (Impact: 4, Effort: 4, Fit: 5, Diff: 2, Risk: 3, Confidence: medium).
- [ ] P1: Add launch-agent self-heal actions (restart/reinstall + open logs) exposed via `doctor`/CLI and menu bar (Impact: 4, Effort: 3, Fit: 4, Diff: 2, Risk: 2, Confidence: medium).
- [ ] P1: Append context-log notes for pause/resume transitions (user + auto reason) to explain timeline gaps during audits (Impact: 4, Effort: 3, Fit: 4, Diff: 2, Risk: 2, Confidence: medium).
- [ ] P1: Add reusable local pre-push verification helper (`fmt/clippy/test/smoke`) to reduce recurrent CI formatting regressions (Impact: 4, Effort: 2, Fit: 4, Diff: 1, Risk: 1, Confidence: high).
- [ ] P1: Add long-session simulated-time reliability tests (hour-scale cadence invariants without wall-clock waits) (Impact: 4, Effort: 3, Fit: 4, Diff: 1, Risk: 2, Confidence: medium).
- [ ] P1: Add high-frequency mode stress tests for stride+budget interaction and deterministic stop semantics (Impact: 4, Effort: 3, Fit: 4, Diff: 1, Risk: 2, Confidence: medium).
- [ ] P2: Add session metrics counters in CLI + menu bar status (captures/skips/failures/bytes written) (Impact: 3, Effort: 3, Fit: 4, Diff: 2, Risk: 2, Confidence: medium).
- [ ] P2: Add URL scheme / deep-link triggers for scripted actions (Raycast/Alfred/Shortcuts parity) (Impact: 3, Effort: 3, Fit: 3, Diff: 3, Risk: 2, Confidence: medium).
- [ ] P2: Add post-capture action pipeline (CLI hooks) (Impact: 3, Effort: 3, Fit: 3, Diff: 3, Risk: 2, Confidence: medium).
- [ ] P2: Add OCR quick-copy flow (macOS Vision / Live Text) for screenshot text extraction (Impact: 3, Effort: 4, Fit: 3, Diff: 3, Risk: 3, Confidence: low).
- [ ] P2: Add pinned screenshot "reference card" window for always-on-top glance workflows (Impact: 3, Effort: 4, Fit: 3, Diff: 4, Risk: 3, Confidence: low).
- [ ] P2: Add sensitive-data smart redaction assistant before export/share actions (Impact: 4, Effort: 5, Fit: 3, Diff: 4, Risk: 4, Confidence: low).
- [ ] P2: Add S3-compatible upload target support with optional short-link output (Impact: 3, Effort: 4, Fit: 3, Diff: 3, Risk: 3, Confidence: low).
- [ ] P3: Add condensed timeline + search filters (app/time/OCR) for retrieval parity with memory/search tools (Impact: 4, Effort: 5, Fit: 4, Diff: 4, Risk: 4, Confidence: low).
- [ ] P3: Decouple analysis from capture path with bounded async queue + retry drain semantics (pre-req for crash recovery) (Impact: 4, Effort: 5, Fit: 4, Diff: 3, Risk: 4, Confidence: low).
- [ ] P3: Add queue persistence + crash recovery for pending analyses (Impact: 4, Effort: 5, Fit: 4, Diff: 3, Risk: 4, Confidence: low).
- [ ] P3: Add performance baseline harness (CPU/RAM/disk throughput) for 2s and 30ms session presets (Impact: 3, Effort: 4, Fit: 3, Diff: 2, Risk: 2, Confidence: medium).

## Implemented

- 2026-02-11: Pause-state transition hardening: engine now emits paused/resumed events only on effective state transitions, preventing false `Running`/`Auto-resumed` states when multiple auto-pause reasons overlap; menubar watcher callbacks no longer force "running" on raw unlock/permission-restored signals; added stacked auto-pause regression test plus permission-watch transition/dedup fault-injection tests (`src/engine.rs`, `src/permission_watch.rs`, `src/bin/menubar.rs`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, GitHub Actions CI run `21894898836`).
- 2026-02-11: Deterministic mock smoke reliability: `--mock-screenshot` now disables permission/activity auto-pause watchers in CLI runs so host lock/sleep state cannot stall CI/local smoke (`src/main.rs`, `README.md`, `bash scripts/smoke.sh`, `cargo test`, GitHub Actions CI run `21894898836`).
- 2026-02-10: Display sleep/wake auto-pause: auto-pause/resume when the display is asleep/awake (prevents black/off frames), with a new explicit pause reason (`DisplayAsleep`) and watchdog unit tests (src/system_activity.rs, src/activity_watch.rs, src/engine.rs, src/main.rs, src/bin/menubar.rs, README.md, docs/idle-autopause-design.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor`).
- 2026-02-10: Phase A idle auto-pause: auto-pause/resume on screen lock/unlock with explicit engine auto-pause reasons, plus scheduler alignment on resume to prevent “catch-up” burst captures after long pauses (src/system_activity.rs, src/activity_watch.rs, src/engine.rs, src/scheduler.rs, src/permission_watch.rs, src/main.rs, src/bin/menubar.rs, README.md, docs/idle-autopause-design.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor`).
- 2026-02-09: Phase 4 safeguards: require confirmation click before starting high-frequency mode, and add a best-effort session storage cap guardrail (`--max-session-bytes`) enforced in the engine; high-frequency menu preset now includes a default cap (src/engine.rs, src/main.rs, src/bin/menubar.rs, README.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`).
- 2026-02-09: Add Accessibility permission diagnostics (menu + `doctor`) and degrade gracefully when `Option+S` hotkey registration fails; add capture throttling via `--capture-stride` and enable sampling in the high-frequency menu preset (src/permissions.rs, src/bin/menubar.rs, src/main.rs, src/engine.rs, README.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor`).
- 2026-02-09: Menubar onboarding UX: when Screen Recording is blocked, tray shows `Status: Blocked` and capture actions are disabled until permission is granted; hotkey-triggered capture no longer auto-opens System Settings (src/bin/menubar.rs, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`).
- 2026-02-09: Added `--mock-screenshot` CLI mode (mock capture provider) + `scripts/smoke.sh` and wired smoke into CI so capture+context can be verified without Screen Recording permission (src/main.rs, scripts/smoke.sh, .github/workflows/ci.yml, README.md, `bash scripts/smoke.sh`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`).
- 2026-02-09: Added `photographic-memory doctor` for one-shot health diagnostics (permissions, privacy policy parse/status, disk free, launch agent status, log paths) (src/main.rs, src/storage.rs, README.md, `cargo run -- doctor`, `cargo test`).
- 2026-02-09: Added golden-format tests for `context.md` entries (capture + skipped) including multiline-summary flattening (src/context_log.rs, `cargo test`).
- 2026-02-09: Added configurable privacy exclusions via `privacy.toml` (deny foreground apps + skip Chromium private/incognito windows) enforced pre-capture with explicit `CaptureSkipped` engine events and rule-only logging (no window titles/URLs) (src/privacy.rs, src/engine.rs, src/context_log.rs, src/main.rs, src/bin/menubar.rs, README.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`).
- 2026-02-09: Repo hygiene and safety: GitHub-visible `README.md`, gitignore runtime artifacts (`captures/`, `context.md`), and safe `context.template.md` to prevent accidental commits of sensitive logs (README.md, .gitignore, context.template.md, `cargo test`).
- 2026-02-08: OpenAI analyzer now enforces 30s request timeout, bounded retry/backoff on transient 429/5xx/connect/timeout failures, and malformed success-payload fallback summaries (src/analysis.rs, README.md, features.md, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`). Prevents API instability from stalling capture loops.
- 2026-02-08: Added analyzer fault-injection tests covering transient retry recovery, non-retryable 400 handling, explicit timeout behavior, and malformed-payload fallback parsing (src/analysis.rs, `cargo test analysis::tests::retries_transient_http_error_and_succeeds -- --exact`, `cargo test`).
- 2026-02-08: Added engine fault-injection tests for screenshot provider failures and context-log write failures; both now verify deterministic failure accounting without session crash (src/engine.rs, `cargo test`).
- 2026-02-08: Added parser regression tests for human-readable byte-size guardrail input (`1.5GB`, underscores, invalid units) and fixed strict clippy findings across CLI/menu/storage/permissions/context modules (src/main.rs, src/bin/menubar.rs, src/storage.rs, src/permissions.rs, src/context_log.rs, `cargo clippy --all-targets --all-features -- -D warnings`).
- 2026-02-08: Added GitHub Actions CI workflow for `cargo fmt --check`, strict clippy, and tests on push/PR (`.github/workflows/ci.yml`).
- 2026-02-08: Added implementation design docs for idle/screen-lock auto-pause and privacy exclusions to guide next milestone execution (docs/idle-autopause-design.md, docs/privacy-controls-design.md).
- 2026-02-08: Screencapture watchdog aborts hung captures after 10s with actionable guidance (src/screenshot.rs, README.md, features.md, cargo test). Prevents macOS permission prompts from freezing the entire session loop.
- 2026-02-08: Disk guard cleanup events now raise CLI + tray notifications showing deleted file count and freed/remaining space (src/engine.rs, src/main.rs, src/bin/menubar.rs, README.md, features.md, cargo test). Gives operators immediate visibility whenever automatic pruning removes captures.
- 2026-02-08: Disk guard now reclaims the oldest captures before bailing (src/storage.rs, src/engine.rs, README.md, features.md, cargo test). Keeps production machines running by freeing space automatically when the 1 GiB threshold trips.
- 2026-02-08: Menu bar icon reflects capture state (src/bin/menubar.rs, README.md). Adds instant visual cue for running/paused/error.
- 2026-02-08: Finder shortcuts for log and captures (src/bin/menubar.rs, README.md). Restores rapid inspection path when debugging AI output.
- 2026-02-08: Menu exposes file-aware \"Open latest capture\" quick link (src/bin/menubar.rs, README.md). Enables one-click auditing/deletion of the newest screenshot.
- 2026-02-08: Screen recording permission health check blocks sessions until macOS access is granted (src/main.rs, src/bin/menubar.rs, src/permissions.rs, README.md). Prevents silent zero-capture runs and deep-links users to System Settings.
- 2026-02-08: Menu surfaces live Screen Recording status plus Recheck/Open Settings actions (src/bin/menubar.rs, README.md). Gives users an always-on diagnostic panel when macOS revokes access mid-session.
- 2026-02-08: Disk space guard halts capture sessions when free space dips below configurable threshold (src/storage.rs, src/engine.rs, src/main.rs, src/bin/menubar.rs, README.md, cargo test). Prevents runaway storage exhaustion on production laptops.
- 2026-02-08: Permission watchdog auto-pauses sessions when macOS revokes Screen Recording mid-run and resumes when access returns (src/permission_watch.rs, src/main.rs, src/bin/menubar.rs, README.md, features.md, cargo test). Prevents silent capture failures and keeps context logs accurate even when privacy prompts pop mid-session.

## Insights

- Pause/resume correctness matters for always-on capture: without scheduler alignment, long pauses (permission revoked, screen locked) cause “catch-up” bursts of rapid captures on resume, which is both noisy and potentially risky.
- Display sleep can happen without a lock event; a separate auto-pause reason helps avoid capturing black/off frames while keeping session state explainable.
- A local mock HTTP server test harness gives deterministic coverage for API retry/timeout semantics without requiring live OpenAI credentials in CI.
- Real screenshots are gated by macOS Screen Recording entitlement; `--mock-screenshot` + `scripts/smoke.sh` provides a permission-free smoke path for CI/dev verification.
- Global hotkey registration can fail (often due to missing Accessibility permission); the tray app should keep running with the hotkey disabled and surface remediation in-menu instead of hard-crashing.
- High-frequency capture needs explicit throttles; `--capture-stride` provides a low-risk knob to reduce disk churn without removing the “high-frequency” scheduling mode entirely.
- Strict clippy (`-D warnings`) surfaced multiple unit-return and collapsible-if issues that are easy to miss in fast iteration; keeping this gate in CI materially improves maintainability.
- Users rely on the tray icon more than menu text when screens are crowded; color coding makes the current session state legible at a glance and lowers anxiety.
- Rapid access to captures/context is essential when auditing AI summaries or deleting sensitive shots; surfacing these actions from the tray avoids Finder spelunking.
- Showing the newest capture filename directly in the menu reduces guesswork when multiple sessions run per day and encourages immediate cleanup of sensitive frames.
- Apple now re-prompts for Screen Recording access on a roughly monthly cadence, so surfacing a live status plus a one-click recheck keeps trust high when captures suddenly stall.
- Automatic capture pruning keeps sessions alive without user action, but we still need to surface a heads-up (menu toast + Finder link) when files are removed so advanced users trust the cleanup.
- Screencapture can hang silently when macOS loses permission mid-run; adding a watchdog makes failures obvious, but we still need proactive permission flip detection so we can auto-stop before the timeout hits.
- Permission flips now auto-pause/resume sessions, and users immediately see status copy plus icon changes, which keeps trust high; next step is tying these events into analytics so we can measure how often Apple revokes access.
- Silent disk cleanup eroded trust; now that CLI/menu surfaces reclaimed file counts with remaining headroom, operators immediately understand what changed and can archive sensitive captures before they vanish again.
- Effective pause state must be treated as a set of active blockers (user + auto reasons); transition events should only fire when that aggregate state actually changes.
- Mock/CI paths should avoid host-dependent background watchers; otherwise lock/sleep signals can cause nondeterministic hangs unrelated to the path under test.

- Market scan notes (2026-02-09, untrusted): Comparable "screen memory" tools emphasize local-first privacy controls and fast retrieval (OCR/search/timeline). Rewind highlights privacy as a core trust lever, while open-source projects like Screenpipe and Windrecorder position local capture + indexing/search as baseline expectations; screenshot tools like Shottr make OCR quick-copy and pinned references feel table-stakes for power users. Sources: https://www.rewind.ai/, https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/
- Market scan notes (2026-02-11, untrusted): Current baseline still clusters around local/private capture guarantees and fast retrieval plus automation hooks. Screenpipe and Windrecorder continue to market local-first indexed capture/search workflows, while Shottr explicitly documents URL-scheme automation for scripted capture actions; Rewind continues to frame encrypted local capture plus optional private-cloud sync as trust differentiators. Sources: https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/kb/urlschemes, https://www.rewind.ai/pricing

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
