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
  - take screenshot every 30ms for next 10 mins (saved ~1/sec, local analysis only)
  - screen recording diagnostics (status row, re-check, open System Settings)
  - privacy policy status + open/reload policy file
  - pause
  - resume
  - stop
  - open context log and captures directory in Finder
  - open the most recent capture instantly
  - quit
- append-only `context.md` logging
- privacy exclusions via a local policy file (`privacy.toml`): deny listed apps and skip Chromium private/incognito windows (best-effort, rule-only logging)
- OpenAI analyzer integration via Responses API
- OpenAI analyzer safeguards: 30s request timeout, bounded retry/backoff for transient API failures, and malformed-payload fallback summaries
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
- Screen Recording diagnostics live in the menu with a status row plus \"Recheck\" and \"Open Settings\" actions so users can recover after macOS revokes access.
- `Option+S` starts an immediate capture session
- Menu exposes an `Open latest capture` action that stays updated with the newest file name for rapid auditing
- A permission watchdog runs behind the scenes; if macOS revokes Screen Recording mid-session the app auto-pauses, surfaces an error toast, and resumes as soon as access returns so you never unknowingly capture blank frames.
- Only one session runs at a time; starting another shows a status warning
- High-frequency mode (`30ms`) disables API analysis to prevent runaway cost and queue pressure
- High-frequency mode also samples disk writes (`--capture-stride`) to avoid runaway storage churn

## Data Location

When using menu bar mode, files are written to:

- captures: `~/Library/Application Support/photographic-memory/captures`
- context log: `~/Library/Application Support/photographic-memory/context.md`
- privacy policy: `~/Library/Application Support/photographic-memory/privacy.toml`

This repository includes `context.template.md` as a safe reference; real runs write to `context.md` which is gitignored by default.

## Privacy Policy (`privacy.toml`)

Captures can be skipped before a screenshot is taken based on the foreground application and (when supported) browser private/incognito windows.

- Open from the menu bar: `Open privacy policy...`
- Reload after editing: `Reload privacy policy`
- Logging rule: skip reasons are recorded as rule-only strings (no window titles or URLs are logged by the privacy checks)
- Private-window detection: best-effort for Chromium browsers (Google Chrome, Brave, Edge, Chromium). If you need a hard guarantee for Safari, add `Safari` to `deny.apps`.

## CLI Reference

### `immediate`

Capture once immediately and append analysis entry.

Key options:

- `--output-dir <path>` (default: `captures`)
- `--context <path>` (default: `context.md`)
- `--model <name>` (default: `gpt-5`)
- `--prompt <text>` custom analysis prompt
- `--no-analyze` disable API analysis
- `--mock-screenshot` use a mock screenshot provider (writes dummy `.png` files) and skips Screen Recording permission checks (useful for CI/smoke)
- `--filename-prefix <prefix>` (default: `capture`)
- `--min-free-bytes <bytes>` abort capture if free disk under this threshold (default: `1GB`; accepts values like `512MB`, `2GB`)
- `--capture-stride <N>` throttle: only attempt a real capture every N scheduler ticks (default: `1`; useful for high-frequency schedules like `30ms`)
- `--privacy-config <path>` override privacy policy TOML path (default: app data dir)
- `--no-privacy` disable privacy checks (unsafe)

### `run`

Run scheduled captures for a fixed time window.

Key options:

- `--every <duration>` (default: `2s`)
- `--for <duration>` (default: `60m`)
- all options from `immediate`
- `--interactive` to enable `pause/resume/stop` from stdin

Duration format examples: `30ms`, `2s`, `5m`, `1h`.

### `doctor`

Print health diagnostics (permissions, privacy policy parse/status, disk headroom, launch-agent status, and log paths).

## Reliability Design

- Capture and analysis are decoupled through trait abstractions
- API errors do not delete captures
- transient OpenAI API failures retry automatically with bounded backoff; non-retryable errors are surfaced immediately
- Context writes are append-only
- Engine supports explicit control commands (`Pause`, `Resume`, `Stop`)
- Testable core modules isolate scheduler and side effects
- launchd `KeepAlive` enables resilient background operation
- Permission watchdog polls Screen Recording state throughout each session and automatically pauses/resumes (with CLI + menu notifications) when macOS flips the entitlement, preventing silent failures.
- `screencapture` invocations are wrapped in an async watchdog so hung permission prompts fail fast instead of stalling sessions indefinitely
- successful-but-malformed OpenAI payloads are summarized safely instead of failing the capture entry append
- Disk health guard + auto-cleanup: the engine refuses to start a capture cycle when free space under the output directory dips below the configurable threshold (default 1 GiB) and automatically prunes the oldest captures to recover space before failing so macOS disks never fill silently
- When the guard prunes captures, both the CLI and the menu bar surface a real-time toast that calls out how many files were deleted plus the freed/remaining capacity so the operator immediately knows what changed.

## Permissions and Privacy

macOS Screen Recording permission is required for captures.

Security guidance:

- Treat captures as sensitive data
- Use encrypted storage if needed
- Add redaction/allowlist controls before broad rollout
- Both the CLI and menu bar app preflight this permission before starting captures. If access is missing, the app surfaces clear instructions and deep-links to the System Settings > Privacy & Security > Screen Recording pane so the user can resolve it without guessing.

## Project Layout

- `src/main.rs` CLI entrypoint (`immediate`, `run`, `plan`)
- `src/bin/menubar.rs` menu bar app + hotkey (`Option+S`)
- `src/engine.rs` capture orchestration and session state machine
- `src/screenshot.rs` screenshot provider abstraction + `screencapture` implementation
- `src/analysis.rs` analyzer abstraction + OpenAI/local implementations
- `src/context_log.rs` append-only context writer
- `src/storage.rs` disk headroom guard + reclaim logic
- `src/privacy.rs` privacy policy enforcement (`privacy.toml`)
- `scripts/install-launch-agent.sh` / `scripts/uninstall-launch-agent.sh` launchd packaging
- `context.template.md` safe context format template
- `features.md` product spec
- `todo.md` market-inspired backlog
