# Incidents And Learnings

## Entry Schema
- Date
- Trigger
- Impact
- Root Cause
- Fix
- Prevention Rule
- Evidence
- Commit
- Confidence

## Entries

- Date: 2026-02-11
  Trigger: Local `scripts/smoke.sh` run hung in mock mode after `session auto-paused: ScreenLocked`.
  Impact: Verification loop stalled and required manual process termination; risk of CI/local false hangs on hosts reporting locked/asleep signals.
  Root Cause: CLI mock mode still spawned permission/activity watchdogs, allowing host lock/sleep state to auto-pause a non-real capture run.
  Fix: Skip permission/activity watcher startup when `--mock-screenshot` is enabled.
  Prevention Rule: Deterministic smoke/CI modes must not depend on host lock/sleep/permission watchers unless explicitly under test.
  Evidence: Initial smoke run stalled until terminated; follow-up `bash scripts/smoke.sh` passed after code fix.
  Commit: 9b4b83b
  Confidence: high

- Date: 2026-02-09
  Trigger: GitHub Actions CI failure on `cargo test` due to a flaky time-based assertion in `engine::tests::context_log_write_failures_are_counted`.
  Impact: CI gate blocked mainline confidence until a follow-up fix landed; risk of future false negatives.
  Root Cause: Test asserted an exact tick count for a very short schedule window (`every=60ms`, `for=125ms`), which can vary across machines/schedulers.
  Fix: Relax assertion to invariants (`failures == total_ticks`, `total_ticks >= 1`) instead of an exact tick count.
  Prevention Rule: For time-based tests, avoid exact-count assertions under sub-second windows; prefer invariants or longer durations with wider tolerances.
  Evidence: GitHub Actions run `21822910706` failed; local `cargo test` passes after fix.
  Commit: bd44cd1
  Confidence: high

- Date: 2026-02-09
  Trigger: GitHub Actions CI failure on `cargo fmt --check` after new code landed.
  Impact: CI was red on main until formatting was corrected; reduced confidence in latest shipped commit.
  Root Cause: Ran `cargo fmt`, then made additional edits and pushed without re-running `cargo fmt --check` (CI enforces fmt).
  Fix: Run `cargo fmt` and push the resulting formatting-only commit.
  Prevention Rule: Always run `cargo fmt --check` immediately before `git push` on formatting-gated repos (or add a local pre-push hook).
  Evidence: GitHub Actions run `21830706709` failed on fmt; local `cargo fmt --check` passes after fix.
  Commit: 1913c9d
  Confidence: high

- Date: 2026-02-10
  Trigger: GitHub Actions CI failure on `cargo fmt --check` after screen-lock auto-pause changes.
  Impact: CI was red on main for commits `aa245a0` and `19cdb65` until formatting was corrected.
  Root Cause: Patched `src/system_activity.rs` after running `cargo fmt`, then pushed without re-running `cargo fmt --check`.
  Fix: Run `cargo fmt` and push the resulting formatting-only commit.
  Prevention Rule: Always run `cargo fmt --check` immediately before `git push`, and re-run it after any last-minute patching.
  Evidence: GitHub Actions runs `21846392790` and `21846413118` failed on fmt; GitHub Actions run `21846436929` succeeded after fix.
  Commit: e6443ae
  Confidence: high
