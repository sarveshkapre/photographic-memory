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
