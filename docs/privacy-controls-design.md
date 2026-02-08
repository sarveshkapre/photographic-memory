# Privacy Controls Design

## Problem

Users need hard guarantees that sensitive applications/sites are excluded from capture, especially for private browsing and regulated data workflows.

## Goals

- App exclusion list (bundle identifiers/process names).
- Browser private/incognito exclusion.
- Clear, inspectable policy behavior in UI and logs.
- Safe defaults with explicit opt-in for broader capture.

## Non-Goals (This Milestone)

- OCR/classifier-based sensitive content detection.
- Cloud policy sync across devices.

## Policy Model

- `deny.apps`: explicit app bundle IDs/process names to skip.
- `deny.browser_private_windows`: boolean, default `true`.
- `deny.domains`: optional list for active tab URL matching when available.
- `allow.override`: explicit allow list that can supersede broad deny categories.

## Enforcement Points

1. Pre-capture foreground app check:
   - if app is denied, skip frame and emit `CaptureSkipped(privacy_rule)`.
2. Browser private-window check:
   - Safari/Chromium adapters detect private mode heuristically.
   - if private mode detected and policy enabled, skip frame.
3. Context logging:
   - skipped events log rule only; never log sensitive titles/URLs.

## UX Requirements

- Menu row: `Privacy: Active (N app rules, private windows excluded)`.
- One-click `Pause Capture` remains highest-priority override.
- Settings file path + edit shortcut exposed in menu for power users.
- First-run notice explains that private-window exclusion is on by default.

## Rollout Plan

- Phase A: app exclusion list from local config file.
- Phase B: private-window exclusion adapters for Safari + Chromium.
- Phase C: optional domain-level filtering with best-effort detection.

## Testing Strategy

- Unit tests for policy matcher precedence (deny vs allow override).
- Integration tests with mocked foreground app/private-window signals.
- Regression test ensuring skipped captures never create image files.
