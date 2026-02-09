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
