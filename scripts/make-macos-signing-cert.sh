#!/usr/bin/env bash
# Makes the certificate release.yml signs the Mac app with when there's no Developer ID, and stores
# it in the repo's MACOS_SIGNING_CERTIFICATE and MACOS_SIGNING_CERTIFICATE_PASSWORD secrets.
# Run it once. macOS remembers a granted permission (Accessibility, Screen Recording) by the app's
# id and this certificate, so a new certificate asks every Mac user for them again: keep the copy
# this leaves in ~/.hyprspace-signing and set the secrets from it if they're ever lost.
#
# Needs openssl and a logged-in gh. Works in Git Bash on Windows too.

set -euo pipefail

REPO="${REPO:-xxashxx-svg/hyprspace}"
KEEP="$HOME/.hyprspace-signing"
P12="$KEEP/macos.p12"
PASSWORD_FILE="$KEEP/macos-password.txt"

if [[ -e "$P12" ]]; then
  echo "$P12 already exists. A new certificate makes every Mac user grant permissions again." >&2
  echo "To put the existing one back in the secrets:" >&2
  echo "  base64 < \"$P12\" | tr -d '\\n' | gh secret set MACOS_SIGNING_CERTIFICATE -R $REPO" >&2
  echo "  gh secret set MACOS_SIGNING_CERTIFICATE_PASSWORD -R $REPO < \"$PASSWORD_FILE\"" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cat >"$WORK/cert.cnf" <<'EOF'
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = HyprSpace
[ext]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
EOF
openssl req -x509 -newkey rsa:2048 -sha256 -days 7300 -nodes -config "$WORK/cert.cnf" \
  -keyout "$WORK/key.pem" -out "$WORK/cert.pem" 2>/dev/null
PASSWORD="$(openssl rand -hex 24)"
mkdir -p "$KEEP"
chmod 700 "$KEEP"
# 3DES and SHA-1 MAC: macOS's `security import` can't read OpenSSL 3's default AES encryption
openssl pkcs12 -export -inkey "$WORK/key.pem" -in "$WORK/cert.pem" -name HyprSpace \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -passout "pass:$PASSWORD" -out "$P12"
printf '%s' "$PASSWORD" >"$PASSWORD_FILE"
chmod 600 "$P12" "$PASSWORD_FILE"

base64 <"$P12" | tr -d '\n' | gh secret set MACOS_SIGNING_CERTIFICATE -R "$REPO"
gh secret set MACOS_SIGNING_CERTIFICATE_PASSWORD -R "$REPO" <"$PASSWORD_FILE"
echo "Set MACOS_SIGNING_CERTIFICATE and MACOS_SIGNING_CERTIFICATE_PASSWORD on $REPO."
echo "Kept a copy in $KEEP. Back it up somewhere safe."
