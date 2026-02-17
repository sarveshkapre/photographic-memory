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

### 2026-02-12T20:01:35Z | Codex execution failure
- Date: 2026-02-12T20:01:35Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-2.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:05:02Z | Codex execution failure
- Date: 2026-02-12T20:05:02Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-3.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:08:31Z | Codex execution failure
- Date: 2026-02-12T20:08:31Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-4.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:11:59Z | Codex execution failure
- Date: 2026-02-12T20:11:59Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-5.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:15:29Z | Codex execution failure
- Date: 2026-02-12T20:15:29Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-6.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:19:00Z | Codex execution failure
- Date: 2026-02-12T20:19:00Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-7.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:22:26Z | Codex execution failure
- Date: 2026-02-12T20:22:26Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-8.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:26:00Z | Codex execution failure
- Date: 2026-02-12T20:26:00Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-9.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:29:34Z | Codex execution failure
- Date: 2026-02-12T20:29:34Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-10.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:33:05Z | Codex execution failure
- Date: 2026-02-12T20:33:05Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-11.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:36:34Z | Codex execution failure
- Date: 2026-02-12T20:36:34Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-12.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:40:00Z | Codex execution failure
- Date: 2026-02-12T20:40:00Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-13.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:43:29Z | Codex execution failure
- Date: 2026-02-12T20:43:29Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-14.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:47:03Z | Codex execution failure
- Date: 2026-02-12T20:47:03Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-15.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:50:29Z | Codex execution failure
- Date: 2026-02-12T20:50:29Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-16.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:54:05Z | Codex execution failure
- Date: 2026-02-12T20:54:05Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-17.log
- Commit: pending
- Confidence: medium

### 2026-02-12T20:57:35Z | Codex execution failure
- Date: 2026-02-12T20:57:35Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-18.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:01:03Z | Codex execution failure
- Date: 2026-02-12T21:01:03Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-19.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:04:32Z | Codex execution failure
- Date: 2026-02-12T21:04:32Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-20.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:08:01Z | Codex execution failure
- Date: 2026-02-12T21:08:01Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-21.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:11:35Z | Codex execution failure
- Date: 2026-02-12T21:11:35Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-22.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:15:09Z | Codex execution failure
- Date: 2026-02-12T21:15:09Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-23.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:18:35Z | Codex execution failure
- Date: 2026-02-12T21:18:35Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-24.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:21:52Z | Codex execution failure
- Date: 2026-02-12T21:21:52Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-25.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:25:06Z | Codex execution failure
- Date: 2026-02-12T21:25:06Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-26.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:28:26Z | Codex execution failure
- Date: 2026-02-12T21:28:26Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-27.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:31:47Z | Codex execution failure
- Date: 2026-02-12T21:31:47Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-28.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:35:20Z | Codex execution failure
- Date: 2026-02-12T21:35:20Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-29.log
- Commit: pending
- Confidence: medium

### 2026-02-12T21:38:45Z | Codex execution failure
- Date: 2026-02-12T21:38:45Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260212-101456-photographic-memory-cycle-30.log
- Commit: pending
- Confidence: medium

### 2026-02-16T22:55:23Z | Codex execution failure
- Date: 2026-02-16T22:55:23Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260216-144104-photographic-memory-cycle-1.log
- Commit: pending
- Confidence: medium

### 2026-02-17T01:42:47Z | Codex execution failure
- Date: 2026-02-17T01:42:47Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260216-144104-photographic-memory-cycle-2.log
- Commit: pending
- Confidence: medium

### 2026-02-17T01:45:56Z | Codex execution failure
- Date: 2026-02-17T01:45:56Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260216-144104-photographic-memory-cycle-3.log
- Commit: pending
- Confidence: medium

### 2026-02-17T01:49:13Z | Codex execution failure
- Date: 2026-02-17T01:49:13Z
- Trigger: Codex execution failure
- Impact: Repo session did not complete cleanly
- Root Cause: codex exec returned a non-zero status
- Fix: Captured failure logs and kept repository in a recoverable state
- Prevention Rule: Re-run with same pass context and inspect pass log before retrying
- Evidence: pass_log=logs/20260216-144104-photographic-memory-cycle-4.log
- Commit: pending
- Confidence: medium
