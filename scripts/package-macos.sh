#!/usr/bin/env bash
# macOS release of the GPUI app, on an Apple silicon Mac. Writes to target/package:
#   HyprSpace.app                       the bundle (apps/hyprspace/package/macos/Info.plist)
#   HyprSpace.app.tar.gz                what both updaters install (latest.json's darwin-aarch64)
#   HyprSpace_<version>_aarch64.dmg     the download, with the drag-to-Applications layout
# The names are the ones every Tauri release used, so the feed and the website keep working.
#
# Signing follows release.yml's Apple secrets, all optional:
#   APPLE_CERTIFICATE (base64 .p12), APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY
#     sign with the Developer ID, hardened runtime on. Without them the app is signed ad hoc,
#     which Apple silicon needs to launch it at all; Gatekeeper then wants right-click, Open.
#   APPLE_ID, APPLE_PASSWORD (app-specific), APPLE_TEAM_ID
#     notarize and staple the app and the dmg, which needs the Developer ID signature too.
# The updater signature (minisign, latest.json) is separate: release.yml signs the tarball after.
#
# Adapted from zeron's scripts/package-macos.sh.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# the workspace's version, the first top-level `version =` in the root manifest
VERSION="$(grep -m1 '^version = ' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
OUT="$ROOT/target/package"
APP="$OUT/HyprSpace.app"
TARBALL="$OUT/HyprSpace.app.tar.gz"
DMG="$OUT/HyprSpace_${VERSION}_aarch64.dmg"

cargo build --release --locked -p hyprspace

rm -rf "$APP" "$TARBALL" "$DMG"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m 755 target/release/hyprspace "$APP/Contents/MacOS/hyprspace"
sed "s/__VERSION__/$VERSION/g" apps/hyprspace/package/macos/Info.plist >"$APP/Contents/Info.plist"
cp apps/hyprspace/assets/hyprspace.icns "$APP/Contents/Resources/hyprspace.icns"

IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
if [[ -n "${APPLE_CERTIFICATE:-}" && -n "$IDENTITY" ]]; then
  # a throwaway keychain holding the Developer ID, for this build only
  KEYCHAIN="${RUNNER_TEMP:-$(mktemp -d)}/hyprspace-build.keychain-db"
  KEYCHAIN_PASSWORD="$(uuidgen)"
  security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  security set-keychain-settings -lut 21600 "$KEYCHAIN"
  security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
  echo "$APPLE_CERTIFICATE" | base64 --decode >"$OUT/cert.p12"
  security import "$OUT/cert.p12" -k "$KEYCHAIN" -P "${APPLE_CERTIFICATE_PASSWORD:-}" -T /usr/bin/codesign
  rm -f "$OUT/cert.p12"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null
  security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | tr -d '"')
  # hardened runtime and a secure timestamp are what notarization asks for; one Mach-O, so no --deep
  codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
else
  IDENTITY=""
  codesign --force --deep --sign - "$APP"
fi
codesign --verify --strict "$APP"

NOTARIZE=false
[[ -n "$IDENTITY" && -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" ]] && NOTARIZE=true
notarize() {
  xcrun notarytool submit "$1" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID" --wait
}
if $NOTARIZE; then
  # staple the bundle before it goes in the tarball: an update swaps the .app in with no dmg,
  # so the bundle has to carry its own ticket
  ditto -c -k --keepParent "$APP" "$OUT/notarize.zip"
  notarize "$OUT/notarize.zip"
  rm -f "$OUT/notarize.zip"
  xcrun stapler staple "$APP"
fi

# one top-level HyprSpace.app, no AppleDouble files: both updaters unpack it over the old bundle
COPYFILE_DISABLE=1 tar -czf "$TARBALL" -C "$OUT" HyprSpace.app

STAGE="$OUT/dmg"
rm -rf "$STAGE" && mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname HyprSpace -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
if [[ -n "$IDENTITY" ]]; then
  codesign --force --timestamp --sign "$IDENTITY" "$DMG"
fi
if $NOTARIZE; then
  notarize "$DMG"
  xcrun stapler staple "$DMG"
fi

echo "packaged $APP"
echo "packaged $TARBALL"
echo "packaged $DMG"
