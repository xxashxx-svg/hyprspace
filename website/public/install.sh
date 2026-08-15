#!/bin/sh
# HyprSpace installer.
#
#   curl -fsSL https://hyprspace.dev/install.sh | sh
#
# Linux : drops the self-updating AppImage in ~/.local/bin and adds a menu entry.
# macOS : mounts the dmg and copies HyprSpace.app into /Applications.
#
# Nothing here needs root, and nothing else about your machine changes. Set HYPRSPACE_BIN to put the
# Linux binary somewhere other than ~/.local/bin.
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

install_linux() {
  [ "$(uname -m)" = "x86_64" ] || die "Linux builds are x86_64 only right now (yours: $(uname -m))"

  bin="${HYPRSPACE_BIN:-$HOME/.local/bin}"
  tmp=$(mktemp -d)
  # shellcheck disable=SC2064  # expand tmp now, not at trap time
  trap "rm -rf '$tmp'" EXIT

  say "Downloading HyprSpace…"
  fetch "$BASE/HyprSpace-linux-x86_64.AppImage" "$tmp/hyprspace"
  chmod +x "$tmp/hyprspace"
  mkdir -p "$bin"
  mv -f "$tmp/hyprspace" "$bin/hyprspace"

  # best-effort icon, so the menu entry isn't a blank tile. Silently skipped if the AppImage's
  # internals ever move — a missing icon is not worth failing an install over.
  icon=hyprspace
  if (cd "$tmp" && "$bin/hyprspace" --appimage-extract '*.png' >/dev/null 2>&1); then
    src=$(find "$tmp/squashfs-root" -name '*.png' 2>/dev/null | head -1)
    if [ -n "$src" ]; then
      icons="$HOME/.local/share/icons/hicolor/256x256/apps"
      mkdir -p "$icons"
      cp -f "$src" "$icons/hyprspace.png"
    fi
  fi

  apps="$HOME/.local/share/applications"
  mkdir -p "$apps"
  cat >"$apps/hyprspace.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=HyprSpace
Comment=Multi-terminal AI workspace
Exec=$bin/hyprspace %U
Icon=$icon
Terminal=false
Categories=Development;IDE;
StartupWMClass=HyprSpace
EOF
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" 2>/dev/null || true

  say "Installed to $bin/hyprspace"
  case ":$PATH:" in
  *":$bin:"*) note "Run 'hyprspace', or find it in your app menu." ;;
  *) note "$bin isn't on your PATH — add it, or launch from your app menu." ;;
  esac
  # the single most common reason an AppImage refuses to start on a fresh distro
  if command -v ldconfig >/dev/null 2>&1 && ! ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; then
    note "Heads up: AppImages need FUSE 2 (apt install libfuse2 / dnf install fuse-libs)."
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
Linux) install_linux ;;
Darwin) install_mac ;;
*) die "unsupported OS: $(uname -s). Windows: irm https://hyprspace.dev/install.ps1 | iex" ;;
esac
