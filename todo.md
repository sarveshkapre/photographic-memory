# TODO: Features Inspired by Similar Screenshot/Memory Apps

Last updated: 2026-02-09

## P0 - Trust, Privacy, and Always-On Reliability

- [ ] App/website exclusion list + incognito/private-window auto-exclusion
  - Why: users need confidence that sensitive surfaces are never captured.
  - Inspired by: Rewind privacy controls and private browsing exclusion.
  - Source: https://www.rewind.ai/ and https://rewinds.sh/privacy

- [ ] One-click pause + visible "capturing now" indicator in menu bar
  - Why: user should always know capture state and have instant override.
  - Inspired by: Rewind menu bar capture toggles in changelog.
  - Source: https://www.rewind.ai/changelog

- [ ] Auto-pause on sleep/idle/static-screen and maintenance when idle
  - Why: reduce useless captures, battery/CPU usage, and storage growth.
  - Inspired by: Windrecorder auto-pause and idle maintenance.
  - Source: https://github.com/yuka-friends/Windrecorder

- [ ] Startup on login + self-heal background agent diagnostics
  - Why: app should keep running without Terminal and recover from crashes.
  - Inspired by: Windrecorder "run on startup" best practice.
  - Source: https://github.com/yuka-friends/Windrecorder

## P1 - Capture UX and Editing Power

- [ ] Pin screenshot as always-on-top floating reference card
  - Why: very useful while filling forms, coding, or copying values.
  - Inspired by: Shottr pinned screenshots.
  - Source: https://shottr.cc/

- [ ] OCR quick-copy flow (single hotkey, auto-copy recognized text)
  - Why: faster extraction from screenshots without manual selection loops.
  - Inspired by: Shottr OCR flow.
  - Source: https://shottr.cc/

- [ ] QR detection in screenshot regions
  - Why: handy for dev/debug flows and 2FA onboarding.
  - Inspired by: Shottr OCR & QR.
  - Source: https://shottr.cc/

- [ ] Built-in annotation toolkit (arrow, highlight, text, blur/pixelate)
  - Why: users need immediate markup before sharing/summarizing.
  - Inspired by: Flameshot in-app editing tools.
  - Source: https://flameshot.org/

- [ ] Sensitive-data smart redaction assistant (emails, phones, cards, URLs)
  - Why: reduce privacy leaks before upload/share.
  - Inspired by: Snagit Smart Redact.
  - Source: https://www.techsmith.com/snagit/features/smart-redact/

- [ ] Scrolling capture with fallback path and clear failure guidance
  - Why: long pages/chat logs are common; auto-scroll can fail in edge cases.
  - Inspired by: Shottr + Snagit docs around scrolling capture reliability.
  - Source: https://shottr.cc/ and https://support.techsmith.com/hc/en-us/articles/203731338-Unable-to-Complete-Scrolling-Capture-in-Snagit

## P2 - Automation and Integrations

- [ ] URL scheme/deep-link API for scripted actions
  - Why: unlock Raycast/Alfred/Shortcuts workflows and automation.
  - Inspired by: Shottr URL Schemes.
  - Source: https://shottr.cc/kb/urlschemes

- [ ] Post-capture action pipeline (CLI hooks)
  - Why: users can chain custom workflows (compress, upload, notify, index).
  - Inspired by: ShareX Actions.
  - Source: https://getsharex.com/actions

- [ ] Reusable effect presets auto-applied after capture
  - Why: standardize visual style for docs/support screenshots.
  - Inspired by: ShareX image effects.
  - Source: https://getsharex.com/image-effects

- [ ] S3-compatible upload targets + short-link return
  - Why: immediate sharing for teams while keeping storage ownership.
  - Inspired by: Shottr S3 upload support.
  - Source: https://shottr.cc/

## P3 - Memory Intelligence and Search

- [ ] Condensed timeline view (collapse long no-activity gaps)
  - Why: faster rewind to meaningful moments.
  - Inspired by: Rewind condensed timeline release notes.
  - Source: https://www.rewind.ai/changelog

- [ ] Search modes: OCR text, visual tags, app filter, time filter
  - Why: screenshot memory is only valuable if retrieval is fast.
  - Inspired by: Rewind searchable memory + Windrecorder OCR/semantic query UI.
  - Source: https://www.rewind.ai/ and https://github.com/yuka-friends/Windrecorder

- [ ] Session metrics dashboard (captures/hour, GB/month, queue lag, failures)
  - Why: users need transparency into cost/performance tradeoffs.
  - Inspired by: Rewind storage/CPU guidance and Windrecorder activity stats.
  - Source: https://www.rewind.ai/ and https://github.com/yuka-friends/Windrecorder

## Next 5 to Build (Recommended)

- [ ] 1. Exclusion list + private window exclusion
- [ ] 2. Auto-pause on idle/static + storage guardrails
- [ ] 3. OCR quick-copy + smart redaction
- [ ] 4. URL scheme + action pipeline hooks
- [ ] 5. Condensed timeline with app/time filters
