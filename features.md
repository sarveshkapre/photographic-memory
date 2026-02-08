# Photographic Memory Features

## Product Goal

Build a macOS-first app (CLI + menu bar) that continuously captures screenshots, analyzes them with ChatGPT, and appends durable memory to `context.md` so work context is never lost.

## Core User Promise

- User always knows capture state (`Running`, `Paused`, `Stopped`)
- User can trigger capture quickly (`Option+S` and menu bar)
- User can trust output persistence (image + context entry written safely)
- User can trust failures are visible and recoverable

## Feature Specification

### 1) Immediate Screenshot

- User action: click `Immediate Screenshot` in menu bar or press `Option+S`
- Expected UX:
  - Menu bar icon animates briefly (capture pulse)
  - Toast/notification: `Screenshot captured`
  - Recent capture row updates in menu drop-down
  - `context.md` receives a new entry within a few seconds
- System behavior:
  - Capture screen to disk
  - Enqueue analysis job
  - Append summary to `context.md` with timestamp + file path
- What can go wrong:
  - Screen recording permission missing
  - Disk write failure
  - API timeout/failure
- Reliability response:
  - If permission missing: show one-click instructions and keep app in `Blocked` state
  - If disk fails: hard error toast + retry suggestion
  - If API fails: append failure note to `context.md` and keep capture file

### 2) Scheduled Capture: Every 2s for 60m

- User action: select preset in menu
- Expected UX:
  - Session banner in menu: `Running • 2s • 60m remaining`
  - Live counters: `Captured`, `Queued`, `Analyzed`, `Errors`
  - Remaining time countdown updates every second
- System behavior:
  - Starts capture scheduler immediately
  - Writes images to session folder
  - Runs analysis in bounded queue
- What can go wrong:
  - Queue buildup due to slow API
  - Network instability
- Reliability response:
  - Backpressure controls and queue limits
  - Retry with exponential backoff
  - Degrade gracefully: continue capturing, mark analysis as pending

### 3) High Frequency Capture: Every 30ms for 10m

- User expectation (important): user expects "very detailed memory"
- Practical reality:
  - 30ms equals ~33 FPS and is not practical for per-frame API analysis
- Expected UX:
  - App warns before start: `This is video-like capture. AI analysis will be sampled.`
  - User chooses: `Proceed with sampled analysis` or `Lower frequency`
- System behavior:
  - Capture stream to disk (or burst chunks)
  - Analyze sampled frames (for example every 1-2s) and scene-change keyframes
  - Append sampled summaries + pointers to batch paths in `context.md`
- What can go wrong:
  - Massive storage growth
  - CPU pressure
  - API cost spikes
- Reliability response:
  - Hard guardrails: storage budget, queue budget, per-session cap
  - Auto-throttle with user-visible reason
  - Auto-pause when system pressure crosses threshold

### 4) Pause / Resume / Stop / Quit

- Pause expected UX:
  - Status flips to `Paused` instantly
  - Next capture timer freezes
  - Queue processing continues unless user selects `Pause All`
- Resume expected UX:
  - Status flips to `Running`
  - Timer restarts cleanly
- Stop expected UX:
  - Capture stops immediately
  - In-flight analysis drains with progress shown
  - Final summary shown (`N captures`, `M analyzed`, `K failed`)
- Quit expected UX:
  - If active session exists, confirmation dialog:
    `Quit and stop session` / `Keep running in background` (for future agent mode)

### 5) Context Log (`context.md`)

- Each capture appends:
  - capture number
  - timestamp (UTC)
  - screenshot file path
  - concise analysis summary
- Expected UX:
  - `Open context.md` action from menu
  - `Jump to latest entry`
- Reliability expectations:
  - Append-only write strategy
  - No truncation on failure
  - If write fails, persist pending entries and retry

### 6) Menu Bar Status and Intuition

- Always-visible state token:
  - `Idle`
  - `Running`
  - `Paused`
  - `Blocked (Permission)`
  - `Error`
- User should always see:
  - current mode
  - interval and remaining duration
  - health indicators

## Detailed Implementation Plan

### Phase 1: Reliable CLI Core (Now)

- Build Rust capture engine
- Support immediate and scheduled capture
- Add pause/resume/stop controls for CLI interactive sessions
- Add analyzer abstraction (OpenAI + local fallback)
- Append structured logs to `context.md`
- Add unit tests for scheduler, context writer, analysis parsing, engine stop behavior

### Phase 2: macOS Menu Bar App

- Add menu bar shell with status text and control items
- Bind controls to shared capture engine
- Add session counters and live updates
- Add global hotkey `Option+S`

### Phase 3: Safety and Scale

- Queue persistence and crash recovery
- Storage budget manager and cleanup tools
- High-frequency sampling mode
- Backoff, retries, dead-letter handling for failed analyses

### Phase 4: UX and Trust

- First-run onboarding for permissions
- Clear error cards with actionable fix steps
- Session timeline view
- One-click open to latest screenshot/context entry

### Phase 5: Quality Bar

- Integration tests with mock screenshot provider and mock analyzer
- Golden tests for `context.md` format
- Fault injection tests (disk full, API timeout, permission denied)
- Performance tests for long-running sessions

## Acceptance Criteria

- User can run one command and produce analyzed entries in `context.md`
- Failures are visible and do not silently drop work
- Session state transitions are deterministic and test-covered
- Menu bar interactions feel immediate and predictable
