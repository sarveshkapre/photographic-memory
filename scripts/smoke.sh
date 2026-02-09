#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

captures_dir="$tmp_dir/captures"
context_path="$tmp_dir/context.md"

echo "Smoke: immediate (mock screenshot, no analyze, no privacy)"
cargo run --quiet --bin photographic-memory --manifest-path "$ROOT/Cargo.toml" -- \
  immediate \
  --no-analyze \
  --mock-screenshot \
  --no-privacy \
  --min-free-bytes 0 \
  --output-dir "$captures_dir" \
  --context "$context_path" \
  --filename-prefix smoke

test -f "$context_path"
grep -q "## Capture 1" "$context_path"

immediate_count="$(ls -1 "$captures_dir" 2>/dev/null | wc -l | tr -d ' ')"
test "$immediate_count" -eq 1

echo "Smoke: scheduled (mock screenshot, no analyze, no privacy)"
captures_dir_2="$tmp_dir/captures2"
context_path_2="$tmp_dir/context2.md"

cargo run --quiet --bin photographic-memory --manifest-path "$ROOT/Cargo.toml" -- \
  run \
  --every 50ms \
  --for 250ms \
  --no-analyze \
  --mock-screenshot \
  --no-privacy \
  --min-free-bytes 0 \
  --output-dir "$captures_dir_2" \
  --context "$context_path_2" \
  --filename-prefix smoke

test -f "$context_path_2"
grep -q "## Capture 1" "$context_path_2"

scheduled_count="$(ls -1 "$captures_dir_2" 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$scheduled_count" -lt 2 ]]; then
  echo "Expected at least 2 scheduled captures, got $scheduled_count" >&2
  exit 1
fi

echo "PASS: smoke"
