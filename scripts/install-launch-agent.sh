#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AGENT_ID="com.sarvesh.photographic-memory"
PLIST_PATH="$HOME/Library/LaunchAgents/${AGENT_ID}.plist"
BIN_PATH="$REPO_ROOT/target/release/menubar"

mkdir -p "$HOME/Library/LaunchAgents"

cargo build --release --bin menubar --manifest-path "$REPO_ROOT/Cargo.toml"

cat > "$PLIST_PATH" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>${AGENT_ID}</string>
    <key>ProgramArguments</key>
    <array>
      <string>${BIN_PATH}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${REPO_ROOT}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>${HOME}/Library/Logs/photographic-memory.log</string>
    <key>StandardErrorPath</key>
    <string>${HOME}/Library/Logs/photographic-memory.err.log</string>
    <key>EnvironmentVariables</key>
    <dict>
      <key>PATH</key>
      <string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
    </dict>
  </dict>
</plist>
PLIST

launchctl bootout "gui/$(id -u)/${AGENT_ID}" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$(id -u)" "$PLIST_PATH"
launchctl kickstart -k "gui/$(id -u)/${AGENT_ID}"

echo "Installed and started ${AGENT_ID}"
echo "Plist: ${PLIST_PATH}"
echo "Logs:  ${HOME}/Library/Logs/photographic-memory.log"
