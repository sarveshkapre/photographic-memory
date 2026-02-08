# Photographic Memory Context

This file is the durable memory log for the app. Runtime capture sessions append entries here.

## Project Context

- Project: `photographic-memory`
- Platform target: macOS
- Interfaces:
  - CLI (implemented)
  - Menu bar app (implemented)
- Core objective: preserve visual work context by storing screenshots and AI analysis summaries.

## Current Architecture (Rust)

- `src/scheduler.rs`
  - interval and duration scheduling logic
- `src/screenshot.rs`
  - screenshot provider abstraction
  - macOS `screencapture` implementation
- `src/analysis.rs`
  - analyzer abstraction
  - OpenAI Responses API analyzer
  - local metadata fallback analyzer
- `src/context_log.rs`
  - append-only markdown context writer
- `src/engine.rs`
  - capture orchestration, control commands, event stream
- `src/main.rs`
  - CLI commands (`immediate`, `run`, `plan`)
- `src/bin/menubar.rs`
  - menu bar UI, hotkey (`Option+S`), and background session control
- `src/storage.rs`
  - disk headroom guard to prevent captures when free space is too low
- `scripts/install-launch-agent.sh`
  - install and start launchd agent for always-on behavior
- `scripts/uninstall-launch-agent.sh`
  - stop and remove launchd agent

## Runtime Entry Template

```md
## Capture <n> at <ISO-8601 UTC>
- Image: <absolute-or-relative-path>
- Summary: <analysis summary>
```

## Reliability Principles

- Persist image first, then analyze
- Append context even when analysis fails
- Never silently drop capture failures
- Keep capture and analysis decoupled for future queue persistence

## Open Product Decisions

- High-frequency mode policy (sampling strategy and storage/cost guardrails)
- Redaction/privacy controls for sensitive screens

## Initial Milestone Notes

- Rust core and tests are implemented first to lock behavior
- Menu bar UX and permission flows are next major milestone
