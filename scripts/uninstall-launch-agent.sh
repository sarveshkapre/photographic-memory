#!/usr/bin/env bash
set -euo pipefail

AGENT_ID="com.sarvesh.photographic-memory"
PLIST_PATH="$HOME/Library/LaunchAgents/${AGENT_ID}.plist"

launchctl bootout "gui/$(id -u)/${AGENT_ID}" >/dev/null 2>&1 || true
rm -f "$PLIST_PATH"

echo "Uninstalled ${AGENT_ID}"
