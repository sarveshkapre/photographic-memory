# Idle And Screen-Lock Auto-Pause Design

## Status

- Phase A (screen-lock auto-pause/resume): implemented on 2026-02-10 (polling-based lock detection; avoids burst "catch-up" captures after resume).
- Sleep/wake auto-pause and static-screen detector: not implemented yet.

## Problem

The app currently captures on a fixed schedule even when the machine is locked, asleep, or visually idle. This creates low-value frames, unnecessary battery/CPU use, and storage churn.

## Goals

- Pause capture automatically when the screen is locked or the system sleeps.
- Optionally pause when visual activity stays static for a configurable window.
- Resume automatically when activity returns.
- Keep behavior explicit in tray/CLI status and context logs.

## Non-Goals (This Milestone)

- Full video-level scene-change detection.
- Multi-display per-screen activity weighting.

## Integration Plan

1. Add `SystemActivityProvider` trait in core engine boundary.
2. Implement macOS provider:
   - subscribe to distributed notifications for session lock/unlock
   - monitor IOKit power notifications for sleep/wake
3. Add optional static-screen detector:
   - sample pixel hash for every Nth capture
   - if hash unchanged for `static_window`, emit `Pause`
   - emit `Resume` when hash changes
4. Wire provider into `permission_watch`-style loop and unify transitions in engine:
   - new `AutoPaused(reason)` and `AutoResumed(reason)` events
5. Surface reasons in menu bar status row and CLI output.
6. Add tests:
   - simulated lock/unlock stream pauses/resumes session
   - static detector triggers at threshold and clears correctly
   - no duplicate pause/resume spam when state is unchanged

## Rollout

- Phase A: lock/sleep auto-pause only (lowest risk).
- Phase B: static-screen auto-pause behind opt-in flag.
- Phase C: default-on static mode after stability data.

## Metrics To Track

- Capture count reduction during idle windows.
- Storage reclaimed per day from avoided captures.
- Auto-pause false-positive rate (manual resume immediately after auto-pause).
