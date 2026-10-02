#!/bin/sh
# HyprSpace installer.
#
#   curl -fsSL https://hyprspace.dev/install.sh | sh
#
# macOS: mounts the dmg and copies HyprSpace.app into /Applications. HyprSpace ships for Windows
# and macOS only; Linux builds stopped in October 2026.
#
# Nothing here needs root, and nothing else about your machine changes.
set -eu

BASE="https://github.com/xxashxx-svg/hyprspace/releases/latest/download"

say() { printf '\033[38;5;110m==>\033[0m %s\n' "$1"; }
note() { printf '    \033[38;5;245m%s\033[0m\n' "$1"; }
die() {
  printf '\033[38;5;203merror:\033[0m %s\n' "$1" >&2
  exit 1
}

fetch() { # url dest
  if command -v curl >/dev/null 2>&1; then
    curl -fL --progress-bar "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -q --show-progress -O "$2" "$1"
  else
    die "need curl or wget to download anything"
  fi
}

install_mac() {
  [ "$(uname -m)" = "arm64" ] || die "macOS builds are Apple Silicon only right now (yours: $(uname -m))"

  tmp=$(mktemp -d)
  mnt="$tmp/mnt"
  # shellcheck disable=SC2064
  trap "hdiutil detach -quiet '$mnt' >/dev/null 2>&1 || true; rm -rf '$tmp'" EXIT

  say "Downloading HyprSpace…"
  fetch "$BASE/HyprSpace-macos-aarch64.dmg" "$tmp/HyprSpace.dmg"

  say "Installing…"
  mkdir -p "$mnt"
  hdiutil attach -quiet -nobrowse -mountpoint "$mnt" "$tmp/HyprSpace.dmg"
  app=$(find "$mnt" -maxdepth 1 -name '*.app' | head -1)
  [ -n "$app" ] || die "no .app inside the disk image"

  dest=/Applications
  [ -w "$dest" ] || dest="$HOME/Applications"
  mkdir -p "$dest"
  rm -rf "$dest/$(basename "$app")"
  cp -R "$app" "$dest/"

  say "Installed to $dest/$(basename "$app")"
  note "Open it from Launchpad, or: open -a HyprSpace"
}

case "$(uname -s)" in
Linux) die "HyprSpace ships for Windows and macOS only. Linux builds stopped in October 2026." ;;
Darwin) install_mac ;;
*) die "unsupported OS: $(uname -s). Windows: irm https://hyprspace.dev/install.ps1 | iex" ;;
esac
