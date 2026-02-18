# Project Memory

## Objective
- Keep photographic-memory production-ready. Current focus: Photographic Memory. Find the highest-impact pending work, implement it, test it, and push to main.

## Architecture Snapshot
- Rust CLI + menu bar app share a single `CaptureEngine` (`src/engine.rs`) with pluggable providers:
  - screenshot: `src/screenshot.rs`
  - analysis: `src/analysis.rs`
  - privacy policy: `src/privacy.rs` (pre-capture skips via `privacy.toml`)
  - context logging: `src/context_log.rs`
  - disk guard: `src/storage.rs`

## Open Problems
- Safari private-window detection is not reliably exposed via AppleScript; the current policy enforces private/incognito exclusion only for supported Chromium browsers. For Safari, users should deny the Safari app explicitly via `deny.apps`.

## Recent Decisions
- Template: YYYY-MM-DD | Decision | Why | Evidence (tests/logs) | Commit | Confidence (high/medium/low) | Trust (trusted/untrusted)
- 2026-02-18 | Ship manual scroll screenshot capture in menubar (start/finish flow) with frame stitching, duplicate suppression, and fallback alignment guardrails | Highest-impact pending UX gap for long pages/chat logs; gives app-agnostic full-page capture without browser automation | `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `bash scripts/smoke.sh` | pending | high | trusted
- 2026-02-17 | Move GitHub Actions CI to repository self-hosted runners (`runs-on: self-hosted`) and add explicit macOS/tooling preflight step | Keep CI operational without GitHub-hosted billing while failing fast on misprovisioned runner hosts | Local end-to-end workflow-equivalent run: preflight checks + `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test` + `bash scripts/smoke.sh` | pending | high | trusted
- 2026-02-17 | Append session pause/resume transitions (user + auto trigger) into `context.md` and gate emission to effective state changes only | Make timeline gaps auditable without adding noise from overlapping auto-pause reasons | `cargo test` (`stacked_auto_pause_reasons_only_resume_after_all_clear`, `resume_does_not_burst_captures_after_long_pause`), `bash scripts/smoke.sh` | pending | high | trusted
- 2026-02-11 | Emit pause/resume events only on effective state transitions across combined user + auto pause reasons; suppress intermediate auto-resume UI flips until all blockers clear | Prevent false `Running` states and misleading auto-resume messaging when multiple auto-pause reasons overlap | `cargo test`, new regression `engine::tests::stacked_auto_pause_reasons_only_resume_after_all_clear`, GitHub Actions CI run `21894898836` | 9b4b83b | high | trusted
- 2026-02-11 | Disable permission/activity watchdogs in CLI `--mock-screenshot` mode | Keep smoke/CI deterministic and prevent host lock/sleep permission signals from stalling mock runs | `bash scripts/smoke.sh`, `cargo test`, GitHub Actions CI run `21894898836` | 9b4b83b | high | trusted
- 2026-02-10 | Display sleep/wake auto-pause: poll display sleep state and auto-pause/resume with explicit `DisplayAsleep` reason; add watchdog unit tests | Prevent capturing black/off frames during display sleep while keeping always-on sessions trustworthy and low-noise | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor`, GitHub Actions CI | f5e84b4 | high | trusted
- 2026-02-10 | Phase A idle auto-pause: auto-pause/resume on screen lock/unlock with explicit auto-pause reasons, and align scheduler on resume to avoid “catch-up” burst captures after long pauses | Reduce low-value locked-screen captures and prevent noisy/risky resume spikes after long pauses (permission revoked, lock) | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor` | aa245a0 | high | trusted
- 2026-02-09 | Phase 4 high-frequency safeguards: require explicit confirmation in tray UI and add session-level storage cap guardrail (`--max-session-bytes`) enforced in engine | Prevent accidental runaway high-frequency sessions (disk churn) while keeping a “fast mode” available for debugging | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh` | 433b9f3 | high | trusted
- 2026-02-09 | Menubar onboarding UX: disable capture actions while Screen Recording is blocked; avoid auto-opening System Settings on hotkey presses; keep idle tray state aligned to permission status | Reduce first-run confusion and prevent accidental permission-pane popups; make blocked-state obvious and recoverable from the menu | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` | 632a176 | high | trusted
- 2026-02-09 | Add configurable privacy exclusions (`privacy.toml`) enforced pre-capture with explicit skip events | Trust: prevent sensitive surfaces from being captured at all; match baseline expectations from comparable memory/screenshot tools | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` | 15e5744 | high | trusted
- 2026-02-09 | Rename `readme.md` -> `README.md`, gitignore runtime artifacts (`captures/`, `context.md`), and keep a safe `context.template.md` | Reduce accidental commits of sensitive screenshots/logs; improve GitHub rendering | `cargo test`, repo file rename diff | 34ca4a3 | high | trusted
- 2026-02-09 | Reduce AppleScript overhead in privacy guard (single `osascript` call per tick, tighter timeout) | High-frequency mode needs low overhead; keep privacy enforcement without stalling the scheduler | `cargo test`, GitHub Actions CI | 2312388 | medium | trusted
- 2026-02-09 | Add `--mock-screenshot` + CI smoke so capture+context paths are verifiable without macOS Screen Recording permission | Developer velocity: unblock reliable local/CI verification even on machines without Screen Recording entitlement | `bash scripts/smoke.sh`, `cargo test`, GitHub Actions CI | 93878fa | high | trusted
- 2026-02-09 | Add `photographic-memory doctor` for one-shot health diagnostics (permissions/privacy/disk/launch-agent/logs) | Production readiness: faster debugging of always-on agent failures and permission gating | `cargo run -- doctor`, `cargo test`, GitHub Actions CI | fd1f698 | high | trusted
- 2026-02-09 | Add golden tests for `context.md` entry format (capture + skipped) | Prevent format drift and regressions in newline-flattening behavior | `cargo test` | 03df7f7 | high | trusted
- 2026-02-09 | Add Accessibility permission diagnostics (menu + `doctor`) and graceful hotkey degradation; add `--capture-stride` throttling for high-frequency schedules | Reduce “hotkey silently broken” UX, and prevent runaway disk churn in high-frequency mode while keeping behavior explicit and testable | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `bash scripts/smoke.sh`, `cargo run --bin photographic-memory -- doctor` | 04418f4 | high | trusted

## Mistakes And Fixes
- Template: YYYY-MM-DD | Issue | Root cause | Fix | Prevention rule | Commit | Confidence
- 2026-02-18 | New scroll-stitch duplicate regression test failed repeatedly (`duplicate_frames` stayed zero) | The synthetic gradient fixture was periodic/ambiguous for overlap scoring, so the assertion targeted the fixture artifact instead of the intended behavior | Changed the duplicate test to use identical solid frames and tightened tie-breaking in overlap scoring to prefer larger overlap on equal scores | For image-stitch tests, avoid periodic fixtures when asserting overlap/duplicate logic; use unambiguous fixtures (identical solids or monotonic rows) | pending | high
- 2026-02-11 | `scripts/smoke.sh` hung in mock mode after `session auto-paused: ScreenLocked` on a locked host | CLI mock runs still spawned lock/permission watchdogs, so host activity state could auto-pause a non-real capture run indefinitely | Skip permission/activity watchers when `--mock-screenshot` is enabled | In deterministic smoke/CI code paths, disable host-dependent background watchers unless they are part of the explicit test objective | 9b4b83b | high
- 2026-02-09 | `doctor` disk free check errored when captures dir did not exist | Assumed statvfs target directory existed | Create captures dir best-effort before querying free space | When adding diagnostic commands, test on fresh/empty state and ensure checks create or fall back to an existing parent path | fd1f698 | high
- 2026-02-09 | CI failed due to flaky time-based engine test asserting exact tick count | Short `every`/`for` durations can produce different tick counts across machines/schedulers | Assert invariants (failures == total_ticks) instead of exact tick counts for short schedules | Avoid exact-tick assertions for sub-second schedules; prefer invariants or longer run windows in tests | bd44cd1 | high
- 2026-02-09 | CI failed on `cargo fmt --check` after code edits landed | Ran `cargo fmt` earlier, then made additional code edits and pushed without re-checking formatting | Run `cargo fmt` and re-run `cargo fmt --check` before pushing a formatting-gated commit | Always run `cargo fmt --check` immediately before `git push` (or add a local pre-push hook) | 1913c9d | high
- 2026-02-10 | CI failed on `cargo fmt --check` for screen-lock auto-pause changes | Patched `src/system_activity.rs` after running `cargo fmt`, then pushed without re-running the format check | Run `cargo fmt` and push a formatting-only fix commit | Always run `cargo fmt --check` immediately before `git push`, and re-run it after any last-minute patching | e6443ae | high

## Known Risks
- Foreground app detection uses AppleScript. Failures/timeouts default to skipping capture (safer), but could cause unexpected “all skipped” sessions if AppleScript is blocked/broken on a machine; menu/CLI should make this obvious.
- Screen lock status is best-effort and can report `Unknown`; in that case the session will not auto-pause for lock/unlock transitions.
- Scroll stitching is viewport-vision based (no browser/Slack automation), so dynamic overlays/animated regions can reduce stitch confidence and yield shorter-than-expected composites.

## Market Scan (Untrusted)
- 2026-02-17 | Snapshot: Current reference products continue to emphasize explicit running/paused visibility, privacy-first local capture defaults, and quick controls/hotkeys. Sources: https://screenpi.pe/, https://github.com/yuka-friends/Windrecorder/blob/main/README.md, https://shottr.cc/, https://www.rewind.ai/
- 2026-02-09 | Snapshot: Screen-memory and screenshot tools emphasize local-first privacy controls and fast retrieval (OCR/search/timeline). Sources: https://www.rewind.ai/, https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/
- 2026-02-10 | Snapshot: Baseline expectations cluster around (1) always-on toggle visibility, (2) local-first capture + indexing, and (3) “power user” affordances like OCR quick-copy and URL-scheme automation. Sources: https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/, https://shottr.cc/kb/urlschemes
- 2026-02-11 | Snapshot: Local/private capture guarantees and scriptable actions remain baseline expectations. Screenpipe and Windrecorder continue to position local indexed memory/search as core UX, Shottr documents URL-scheme automation for power workflows, and Rewind continues to emphasize encrypted local capture with optional private cloud. Sources: https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/kb/urlschemes, https://www.rewind.ai/pricing

## Gap Map (Untrusted)
- Parity: explicit paused/blocked/running status states; pre-capture privacy exclusions; reliable always-on agent behavior.
- Weak: session transparency beyond state transitions (capture counters and richer pause diagnostics in the memory stream); idle/static-screen optimizations.
- Missing: fast retrieval UX (timeline/search/OCR); automation endpoints (URL scheme + action hooks); pinned reference/quick OCR copy.
- Differentiator: append-only `context.md` memory stream with OpenAI analysis; CLI-first + menubar shell sharing a single engine.

## Next Prioritized Tasks
- Optional static-screen auto-pause behind an explicit opt-in flag.
- Launch-agent self-heal actions exposed via `doctor` + tray menu.
- Add lightweight session counters (`captures/skips/failures`) to tray status and periodic context notes for faster operator audits.

## Verification Evidence
- Template: YYYY-MM-DD | Command | Key output | Status (pass/fail)
- 2026-02-18 | `cargo fmt --check` | clean after scroll-capture + menubar integration | pass
- 2026-02-18 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings after adding `image` dependency and scroll module | pass
- 2026-02-18 | `cargo test` | first run failed (`scroll_capture::tests::skips_duplicate_frames_with_no_new_rows`), follow-up run passed after fixture + tie-break fixes (`38` lib tests + `2` main tests) | pass
- 2026-02-18 | `bash scripts/smoke.sh` | PASS: smoke (mock screenshot flow unchanged after scroll capture feature) | pass
- 2026-02-17 | `if [[ \"$(uname -s)\" != \"Darwin\" ]]; then exit 1; fi; xcode-select -p; command -v bash; command -v git; cargo fmt --check; cargo clippy --all-targets --all-features -- -D warnings; cargo test; bash scripts/smoke.sh` | preflight checks passed; clippy clean; tests passed (`34` + `2`); smoke PASS | pass
- 2026-02-17 | `cargo fmt --check` | clean | pass
- 2026-02-17 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings (after transient local cargo lock waits) | pass
- 2026-02-17 | `cargo test` | failed: `engine::tests::resume_does_not_burst_captures_after_long_pause` + `engine::tests::stacked_auto_pause_reasons_only_resume_after_all_clear` (`context.md` missing due moved tempdir) | fail
- 2026-02-17 | `cargo test` | 34 tests passed after tempdir lifetime fix in regression tests | pass
- 2026-02-17 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-11 | `cargo fmt --check` | clean | pass
- 2026-02-11 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-11 | `cargo test` | 33 tests passed | pass
- 2026-02-11 | `bash scripts/smoke.sh` | first run hung after `session auto-paused: ScreenLocked` in mock mode | fail
- 2026-02-11 | `bash scripts/smoke.sh` | PASS: smoke (after mock-mode watcher fix) | pass
- 2026-02-11 | `cargo run --bin photographic-memory -- doctor` | prints health report | pass
- 2026-02-11 | GitHub Actions CI run `21894898836` | conclusion: success | pass
- 2026-02-10 | `cargo fmt --check` | clean | pass
- 2026-02-10 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-10 | `cargo test` | 32 tests passed | pass
- 2026-02-10 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-10 | `cargo run --bin photographic-memory -- doctor` | prints health report | pass
- 2026-02-10 | GitHub Actions CI run `21854247174` | conclusion: success | pass
- 2026-02-10 | GitHub Actions CI run `21854289244` | conclusion: success | pass
- 2026-02-10 | `cargo fmt` | formatted | pass
- 2026-02-10 | `cargo fmt --check` | clean | pass
- 2026-02-10 | `cargo test` | 30 tests passed | pass
- 2026-02-10 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-10 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-10 | `cargo run --bin photographic-memory -- doctor` | prints health report | pass
- 2026-02-10 | GitHub Actions CI run `21846436929` | conclusion: success | pass
- 2026-02-09 | `cargo fmt` | formatted | pass
- 2026-02-09 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-09 | `cargo test` | 22 tests passed | pass
- 2026-02-09 | `cargo run --bin photographic-memory -- plan` | prints roadmap | pass
- 2026-02-09 | `cargo run --bin photographic-memory -- immediate --no-analyze ...` | Screen Recording permission denied (expected gating) | pass (blocked by permission)
- 2026-02-09 | GitHub Actions CI run `21815357626` | conclusion: success | pass
- 2026-02-09 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-09 | `cargo run --bin photographic-memory -- doctor` | prints health report | pass
- 2026-02-09 | `cargo test` | 25 tests passed | pass
- 2026-02-09 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-09 | `cargo fmt` | formatted | pass
- 2026-02-09 | `cargo test` | 26 tests passed | pass
- 2026-02-09 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-09 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-09 | `cargo run --bin photographic-memory -- doctor` | prints report including Screen Recording + Accessibility status | pass
- 2026-02-09 | `cargo fmt --check` | clean | pass
- 2026-02-09 | `cargo test` | 26 tests passed | pass
- 2026-02-09 | `cargo fmt` | formatted | pass
- 2026-02-09 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-09 | `cargo test` | 26 tests passed | pass
- 2026-02-09 | `cargo fmt` | formatted | pass
- 2026-02-09 | `cargo clippy --all-targets --all-features -- -D warnings` | no warnings | pass
- 2026-02-09 | `cargo test` | 27 tests passed | pass
- 2026-02-09 | `bash scripts/smoke.sh` | PASS: smoke | pass
- 2026-02-09 | GitHub Actions CI run `21839646975` | conclusion: success | pass

## Historical Summary
- Keep compact summaries of older entries here when file compaction runs.
