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
- 2026-02-09 | Add configurable privacy exclusions (`privacy.toml`) enforced pre-capture with explicit skip events | Trust: prevent sensitive surfaces from being captured at all; match baseline expectations from comparable memory/screenshot tools | `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings` | 15e5744 | high | trusted
- 2026-02-09 | Rename `readme.md` -> `README.md`, gitignore runtime artifacts (`captures/`, `context.md`), and keep a safe `context.template.md` | Reduce accidental commits of sensitive screenshots/logs; improve GitHub rendering | `cargo test`, repo file rename diff | 34ca4a3 | high | trusted
- 2026-02-09 | Reduce AppleScript overhead in privacy guard (single `osascript` call per tick, tighter timeout) | High-frequency mode needs low overhead; keep privacy enforcement without stalling the scheduler | `cargo test`, GitHub Actions CI | 2312388 | medium | trusted
- 2026-02-09 | Add `--mock-screenshot` + CI smoke so capture+context paths are verifiable without macOS Screen Recording permission | Developer velocity: unblock reliable local/CI verification even on machines without Screen Recording entitlement | `bash scripts/smoke.sh`, `cargo test`, GitHub Actions CI | 93878fa | high | trusted
- 2026-02-09 | Add `photographic-memory doctor` for one-shot health diagnostics (permissions/privacy/disk/launch-agent/logs) | Production readiness: faster debugging of always-on agent failures and permission gating | `cargo run -- doctor`, `cargo test`, GitHub Actions CI | fd1f698 | high | trusted
- 2026-02-09 | Add golden tests for `context.md` entry format (capture + skipped) | Prevent format drift and regressions in newline-flattening behavior | `cargo test` | 03df7f7 | high | trusted

## Mistakes And Fixes
- Template: YYYY-MM-DD | Issue | Root cause | Fix | Prevention rule | Commit | Confidence
- 2026-02-09 | `doctor` disk free check errored when captures dir did not exist | Assumed statvfs target directory existed | Create captures dir best-effort before querying free space | When adding diagnostic commands, test on fresh/empty state and ensure checks create or fall back to an existing parent path | fd1f698 | high
- 2026-02-09 | CI failed due to flaky time-based engine test asserting exact tick count | Short `every`/`for` durations can produce different tick counts across machines/schedulers | Assert invariants (failures == total_ticks) instead of exact tick counts for short schedules | Avoid exact-tick assertions for sub-second schedules; prefer invariants or longer run windows in tests | bd44cd1 | high

## Known Risks
- Foreground app detection uses AppleScript. Failures/timeouts default to skipping capture (safer), but could cause unexpected “all skipped” sessions if AppleScript is blocked/broken on a machine; menu/CLI should make this obvious.

## Market Scan (Untrusted)
- 2026-02-09 | Snapshot: Screen-memory and screenshot tools emphasize local-first privacy controls and fast retrieval (OCR/search/timeline). Sources: https://www.rewind.ai/, https://github.com/mediar-ai/screenpipe, https://github.com/yuka-friends/Windrecorder, https://shottr.cc/

## Next Prioritized Tasks
- Idle/screen-lock auto-pause (lock/sleep first; static-screen optional) to reduce low-value capture churn.
- Add launch-agent diagnostics action so startup-on-login failures self-heal without manual plist spelunking.
- Add queue/latency counters to better expose capture vs analysis backlog.

## Verification Evidence
- Template: YYYY-MM-DD | Command | Key output | Status (pass/fail)
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

## Historical Summary
- Keep compact summaries of older entries here when file compaction runs.
