# Photographic Memory

A Rust macOS project that captures screenshots, analyzes each capture with ChatGPT, and appends persistent memory to `context.md`.

## Why

You lose context when windows change, tasks switch, or sessions restart. This app keeps a durable timeline of what was on screen and what it meant.

## Current Status

Implemented now:

- Rust CLI capture engine
- Rust menu bar app (`menubar` binary)
- global hotkey `Option+S` for immediate screenshot
- menu options:
  - immediate screenshot
  - take screenshot every 2s for next 60 mins
  - take screenshot every 30ms for next 10 mins (AI sampled/local analysis only)
  - pause
  - resume
  - stop
  - quit
- append-only `context.md` logging
- OpenAI analyzer integration via Responses API
- metadata fallback analyzer when `OPENAI_API_KEY` is not set
- launchd scripts so app can stay running after Terminal closes
- unit tests across scheduler, engine, analysis extraction, and context log

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

### 3) Run CLI (one-off or scheduled)

Immediate capture:

```bash
cargo run -- immediate --output-dir captures --context context.md --model gpt-5
```

Scheduled capture:

```bash
cargo run -- run --every 2s --for 60m --interactive
```

Interactive commands while running:

- `pause`
- `resume`
- `stop`

### 4) Run menu bar app

```bash
cargo run --release --bin menubar
```

### 5) Keep it alive after closing Terminal

Install as launchd user agent:

```bash
./scripts/install-launch-agent.sh
```

This will:

- build `menubar` in release mode
- install `~/Library/LaunchAgents/com.sarvesh.photographic-memory.plist`
- start the background agent with `KeepAlive=true`

Uninstall:

```bash
./scripts/uninstall-launch-agent.sh
```

### 6) Test

```bash
cargo test
```

## Menu Bar Behavior

- Status text always shows current state (`Idle`, `Running`, `Paused`, `Done`, `Error`)
- Menu bar icon is color-coded for quick scanning (gray idle, green running, yellow paused, red error)
- `Option+S` starts an immediate capture session
- Only one session runs at a time; starting another shows a status warning
- High-frequency mode (`30ms`) disables API analysis to prevent runaway cost and queue pressure

## Data Location

When using menu bar mode, files are written to:

- captures: `~/Library/Application Support/photographic-memory/captures`
- context log: `~/Library/Application Support/photographic-memory/context.md`

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

## Reliability Design

- Capture and analysis are decoupled through trait abstractions
- API errors do not delete captures
- Context writes are append-only
- Engine supports explicit control commands (`Pause`, `Resume`, `Stop`)
- Testable core modules isolate scheduler and side effects
- launchd `KeepAlive` enables resilient background operation

## Permissions and Privacy

macOS Screen Recording permission is required for captures.

Security guidance:

- Treat captures as sensitive data
- Use encrypted storage if needed
- Add redaction/allowlist controls before broad rollout

## Project Files

- `/Users/sarvesh/code/photographic-memory/src/main.rs`
- `/Users/sarvesh/code/photographic-memory/src/bin/menubar.rs`
- `/Users/sarvesh/code/photographic-memory/src/engine.rs`
- `/Users/sarvesh/code/photographic-memory/src/analysis.rs`
- `/Users/sarvesh/code/photographic-memory/src/scheduler.rs`
- `/Users/sarvesh/code/photographic-memory/src/context_log.rs`
- `/Users/sarvesh/code/photographic-memory/src/screenshot.rs`
- `/Users/sarvesh/code/photographic-memory/scripts/install-launch-agent.sh`
- `/Users/sarvesh/code/photographic-memory/scripts/uninstall-launch-agent.sh`
- `/Users/sarvesh/code/photographic-memory/features.md`
- `/Users/sarvesh/code/photographic-memory/context.md`
- `/Users/sarvesh/code/photographic-memory/todo.md`
