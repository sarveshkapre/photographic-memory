# Photographic Memory

A Rust macOS project that captures screenshots, analyzes each capture with ChatGPT, and appends persistent memory to `context.md`.

## Why

You lose context when windows change, tasks switch, or sessions restart. This app keeps a durable timeline of what was on screen and what it meant.

## Current Status

Implemented now:

- Rust CLI capture engine
- `immediate` screenshot mode
- scheduled `run` mode with configurable interval and duration
- interactive controls in CLI sessions: `pause`, `resume`, `stop`
- append-only `context.md` logging
- OpenAI analyzer integration via Responses API
- metadata fallback analyzer when `OPENAI_API_KEY` is not set
- unit tests across scheduler, engine, analysis extraction, and context log

Planned next:

- macOS menu bar app
- global hotkey `Option+S`
- menu presets (`Immediate`, `Every 2s for 60m`, `Every 30ms for 10m` with guardrails)
- richer live status in menu bar

## Quick Start

### 1) Prerequisites

- macOS
- Rust toolchain (`cargo`)
- `screencapture` command available (default on macOS)
- optional: `OPENAI_API_KEY` for ChatGPT analysis

### 2) Build

```bash
cargo build
```

### 3) Run Immediate Capture

```bash
cargo run -- immediate --output-dir captures --context context.md --model gpt-5
```

### 4) Run Scheduled Session

```bash
cargo run -- run --every 2s --for 60m --interactive
```

Interactive commands while running:

- `pause`
- `resume`
- `stop`

### 5) Test

```bash
cargo test
```

## CLI Reference

### `immediate`

Capture once immediately and append analysis entry.

Key options:

- `--output-dir <path>` (default: `captures`)
- `--context <path>` (default: `context.md`)
- `--model <name>` (default: `gpt-5`)
- `--prompt <text>` custom analysis prompt
- `--no-analyze` disable API analysis
- `--filename-prefix <prefix>` (default: `capture`)

### `run`

Run scheduled captures for a fixed time window.

Key options:

- `--every <duration>` (default: `2s`)
- `--for <duration>` (default: `60m`)
- all options from `immediate`
- `--interactive` to enable `pause/resume/stop` from stdin

Duration format examples: `30ms`, `2s`, `5m`, `1h`.

### `plan`

Prints the planned menu bar implementation roadmap.

## Reliability Design

- Capture and analysis are decoupled through trait abstractions
- API errors do not delete captures
- Context writes are append-only
- Engine supports explicit control commands (`Pause`, `Resume`, `Stop`)
- Testable core modules isolate scheduler and side effects

## Permissions and Privacy

When menu bar mode is added, macOS Screen Recording permission is required.

Security guidance:

- Treat captures as sensitive data
- Use encrypted storage if needed
- Add redaction/allowlist controls before broad rollout

## Project Files

- `/Users/sarvesh/code/photographic-memory/src/main.rs`
- `/Users/sarvesh/code/photographic-memory/src/engine.rs`
- `/Users/sarvesh/code/photographic-memory/src/analysis.rs`
- `/Users/sarvesh/code/photographic-memory/src/scheduler.rs`
- `/Users/sarvesh/code/photographic-memory/src/context_log.rs`
- `/Users/sarvesh/code/photographic-memory/src/screenshot.rs`
- `/Users/sarvesh/code/photographic-memory/features.md`
- `/Users/sarvesh/code/photographic-memory/context.md`

## Roadmap

1. Add menu bar shell and state model
2. Connect menu actions to shared capture engine
3. Add global shortcut `Option+S`
4. Add high-frequency sampling safeguards
5. Add queue persistence and crash recovery
6. Add first-run permission onboarding
