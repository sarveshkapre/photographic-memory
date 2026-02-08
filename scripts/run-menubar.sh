#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo run --release --bin menubar --manifest-path "$REPO_ROOT/Cargo.toml"
