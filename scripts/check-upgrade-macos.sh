#!/usr/bin/env bash
# Let the Tauri app's own updater install the GPUI app, then check what the user ends up with.
# CI only (.github/workflows/upgrade-test.yml): it puts a HyprSpace.app in ~/Applications and
# writes ~/.hyprspace, so it refuses to run anywhere but a throwaway runner.
#
#   scripts/check-upgrade-macos.sh <tauri HyprSpace.app> <feed folder> <out folder> [port]
#
# The Tauri app is built from the v0.21.1 tag with its updater feed at http://127.0.0.1:<port>; the
# feed folder holds latest.json and the signed HyprSpace.app.tar.gz it names, and this script
# serves it. The app gets a project, starts, and its updater has to download the tarball, verify
# it, swap the bundle and relaunch into the GPUI app, which has to bring the project over. Logs,
# the state files and screenshots land in the out folder.

set -euo pipefail

if [[ "${GITHUB_ACTIONS:-}" != "true" ]]; then
  echo "This installs a HyprSpace.app and writes ~/.hyprspace. It only runs on a CI runner." >&2
  exit 1
fi

OLD="$1"
FEED="$(cd "$2" && pwd)"
mkdir -p "$3"
OUT="$(cd "$3" && pwd)"
PORT="${4:-8765}"
APP="$HOME/Applications/HyprSpace.app"
PROJECT="${RUNNER_TEMP:-/tmp}/upgrade-demo"
V2="$HOME/.hyprspace/v2"
NATIVE="$HOME/.hyprspace/native"
VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$FEED/latest.json")"
TARBALL="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["platforms"]["darwin-aarch64"]["url"].rsplit("/",1)[-1])' "$FEED/latest.json")"
plist() { /usr/libexec/PlistBuddy -c "Print :$1" "$APP/Contents/Info.plist"; }
# the GPUI app, started by the Tauri app's relaunch or by `open`
gpui() { pgrep -f "^$APP/Contents/MacOS/hyprspace( |$)"; }
no_tauri() { ! pgrep -f "MacOS/hyprspace-tauri"; }
swapped() { [[ "$(plist CFBundleExecutable)" == hyprspace ]]; }

check() {
  local what="$1"
  shift
  if "$@"; then echo "ok   $what"; else echo "FAILED: $what" >&2; exit 1; fi
}

wait_until() {
  local what="$1" seconds="$2"
  shift 2
  local deadline=$((SECONDS + seconds))
  until "$@" >/dev/null 2>&1; do
    if ((SECONDS > deadline)); then echo "FAILED: $what (waited ${seconds}s)" >&2; exit 1; fi
    sleep 0.5
  done
  echo "ok   $what"
}

# evidence only: screen capture may lack the permission on a runner
shot() { screencapture -x "$OUT/$1.png" 2>/dev/null || echo "no screenshot ($1)"; }

SERVER=""
finish() {
  shot 3-end
  cp -R "$V2" "$OUT/v2" 2>/dev/null || true
  cp "$NATIVE/state.json" "$OUT/" 2>/dev/null || true
  ls -la "$APP/Contents/MacOS" >"$OUT/bundle.txt" 2>&1 || true
  cp "$APP/Contents/Info.plist" "$OUT/" 2>/dev/null || true
  pkill -f "^$APP/Contents/MacOS/" 2>/dev/null || true
  if [[ -n "$SERVER" ]]; then kill "$SERVER" 2>/dev/null || true; fi
}
trap finish EXIT

# 1. the Tauri app, in an Applications folder the way a user has it
mkdir -p "$(dirname "$APP")"
rm -rf "$APP"
ditto "$OLD" "$APP"
OLD_VERSION="$(plist CFBundleShortVersionString)"
check "the Tauri app is in $APP" test -x "$APP/Contents/MacOS/hyprspace-tauri"
echo "     Tauri app version $OLD_VERSION, feed offers $VERSION"

# 2. a project, in the Tauri app's own store
mkdir -p "$PROJECT" "$V2"
python3 - "$V2" "$PROJECT" "$OLD_VERSION" <<'EOF'
import json, sys, time
v2, project, old = sys.argv[1:]
ws = {"workspaces": [{"id": "p1", "name": "upgrade-demo", "cwd": project, "color": "#3fb6e0",
      "kind": "project", "sessions": [], "lastOpenedAt": int(time.time() * 1000)}], "activeId": "p1"}
json.dump(ws, open(f"{v2}/workspaces.json", "w"))
json.dump({"theme": "iris", "onboarded": True}, open(f"{v2}/settings.json", "w"))
json.dump(old, open(f"{v2}/lastSeenVersion.json", "w"))
EOF
check "no GPUI state yet" test ! -e "$NATIVE/state.json"

# 3. the feed
python3 -u -m http.server "$PORT" --bind 127.0.0.1 --directory "$FEED" >"$OUT/feed.out.log" 2>"$OUT/feed.log" &
SERVER=$!
wait_until "the feed answers" 30 curl -fsS "http://127.0.0.1:$PORT/latest.json"

# 4. start the Tauri app and let its updater work
open "$APP"
sleep 3
shot 1-tauri-app
wait_until "the bundle now runs the GPUI binary" 240 swapped
wait_until "the GPUI app is running from $APP" 120 gpui
wait_until "the Tauri app quit" 60 no_tauri
sleep 15
shot 2-gpui-app

# 5. what the user ends up with
check "the GPUI app is still running 15s later" gpui
check "the Tauri app downloaded $TARBALL from the feed" grep -q "GET /$TARBALL " "$OUT/feed.log"
check "the bundle is version $VERSION (got $(plist CFBundleShortVersionString))" test "$(plist CFBundleShortVersionString)" = "$VERSION"
check "the Tauri binary is gone" test ! -e "$APP/Contents/MacOS/hyprspace-tauri"
check "no Tauri process is left" no_tauri
wait_until "the GPUI app saved its state" 30 test -e "$NATIVE/state.json"
check "the GPUI app has the project $PROJECT and the theme" python3 - "$NATIVE/state.json" "$PROJECT" <<'EOF'
import json, sys
state = json.load(open(sys.argv[1]))
cwds = [(s.get("cwd") or "").rstrip("/") for s in state.get("spaces", [])]
print("     spaces:", cwds, "theme:", state.get("appearance", {}).get("theme"))
sys.exit(0 if cwds.count(sys.argv[2]) == 1 and state["appearance"]["theme"] == "iris" else 1)
EOF
echo "all good: the Tauri app $OLD_VERSION updated itself into the GPUI app $VERSION"
