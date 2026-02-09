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

## Mistakes And Fixes
- Template: YYYY-MM-DD | Issue | Root cause | Fix | Prevention rule | Commit | Confidence

## Known Risks
- Foreground app detection uses AppleScript. Failures/timeouts default to skipping capture (safer), but could cause unexpected “all skipped” sessions if AppleScript is blocked/broken on a machine; menu/CLI should make this obvious.

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

## Historical Summary
- Keep compact summaries of older entries here when file compaction runs.
