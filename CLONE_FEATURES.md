# Clone Feature Tracker

## Context Sources
- README and docs
- TODO/FIXME markers in code
- Test and build failures
- Gaps found during codebase exploration

## Candidate Features To Do

- Auto-pause capture when macOS session locks or screen idle detector trips.
- Configurable privacy filters (domain/app exclusion list and incognito detection).
- OCR quick-copy shortcut with optional sensitive-data redaction presets.
- Permission watchdog that auto-pauses/resumes sessions when Screen Recording access flips mid-run.
- Analyzer health safeguards: add request timeouts, retries, and queue telemetry so API hiccups don't stall captures forever.
- Disk guard notifications: surface toast/log entries when auto-prune deletes captures so users can audit what disappeared.

## Implemented

- 2026-02-08: Screencapture watchdog aborts hung captures after 10s with actionable guidance (src/screenshot.rs, readme.md, features.md, cargo test). Prevents macOS permission prompts from freezing the entire session loop.
- 2026-02-08: Disk guard now reclaims the oldest captures before bailing (src/storage.rs, src/engine.rs, readme.md, features.md, cargo test). Keeps production machines running by freeing space automatically when the 1 GiB threshold trips.
- 2026-02-08: Menu bar icon reflects capture state (src/bin/menubar.rs, readme.md). Adds instant visual cue for running/paused/error.
- 2026-02-08: Finder shortcuts for log and captures (src/bin/menubar.rs, readme.md). Restores rapid inspection path when debugging AI output.
- 2026-02-08: Menu exposes file-aware \"Open latest capture\" quick link (src/bin/menubar.rs, readme.md). Enables one-click auditing/deletion of the newest screenshot.
- 2026-02-08: Screen recording permission health check blocks sessions until macOS access is granted (src/main.rs, src/bin/menubar.rs, src/permissions.rs, readme.md). Prevents silent zero-capture runs and deep-links users to System Settings.
- 2026-02-08: Menu surfaces live Screen Recording status plus Recheck/Open Settings actions (src/bin/menubar.rs, readme.md). Gives users an always-on diagnostic panel when macOS revokes access mid-session.
- 2026-02-08: Disk space guard halts capture sessions when free space dips below configurable threshold (src/storage.rs, src/engine.rs, src/main.rs, src/bin/menubar.rs, readme.md, cargo test). Prevents runaway storage exhaustion on production laptops.

## Insights

- Users rely on the tray icon more than menu text when screens are crowded; color coding makes the current session state legible at a glance and lowers anxiety.
- Rapid access to captures/context is essential when auditing AI summaries or deleting sensitive shots; surfacing these actions from the tray avoids Finder spelunking.
- Showing the newest capture filename directly in the menu reduces guesswork when multiple sessions run per day and encourages immediate cleanup of sensitive frames.
- Apple now re-prompts for Screen Recording access on a roughly monthly cadence, so surfacing a live status plus a one-click recheck keeps trust high when captures suddenly stall.
- Even with the new diagnostics, we still need to monitor for permission flips mid-session and auto-pause/resume captures instead of silently failing mid-recording.
- Automatic capture pruning keeps sessions alive without user action, but we still need to surface a heads-up (menu toast + Finder link) when files are removed so advanced users trust the cleanup.
- Screencapture can hang silently when macOS loses permission mid-run; adding a watchdog makes failures obvious, but we still need proactive permission flip detection so we can auto-stop before the timeout hits.

## Notes
- This file is maintained by the autonomous clone loop.

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
