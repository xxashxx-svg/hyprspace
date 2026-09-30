# Building HyprSpace on macOS

There's no prebuilt Mac download — a Mac app can only be built on a Mac. These steps
produce a `.dmg` you can install. Takes ~15 min the first time (mostly Rust compiling).

## 1. Install the prerequisites

```bash
# Xcode command-line tools (compiler + git)
xcode-select --install

# Rust (accept the defaults, then restart your terminal)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js 20+ (22 recommended) — from https://nodejs.org or:
brew install node
```

## 2. Get the code

```bash
git clone https://github.com/xxashxx-svg/hyprspace.git
cd hyprspace
```

## 3. One local tweak

The repo is set up to build Windows auto-update files that need the maintainer's private
signing key. For a local Mac build, open `src-tauri/tauri.conf.json` and change:

```json
"createUpdaterArtifacts": true,
```
to
```json
"createUpdaterArtifacts": false,
```

Don't commit that change — it's only for your local build. A locally built app won't
auto-update; rebuild from a newer checkout to get changes.

## 4. Build

```bash
npm install
npm run tauri build -- --bundles dmg
```

Your Mac's chip is detected automatically (Apple Silicon → `aarch64`, Intel → `x86_64`).

## 5. Install & run

The `.dmg` lands in:

```
src-tauri/target/release/bundle/dmg/HyprSpace_0.2.0_<arch>.dmg
```

- Open it, drag **HyprSpace** into Applications.
- First launch is blocked because the app isn't Apple-signed — **right-click the app →
  Open → Open** (only needed once). Or: System Settings → Privacy & Security → **Open Anyway**.
- At the activation screen, paste your license key.

## If Screen Recording or another permission stops working

macOS ties permissions like Screen Recording and Accessibility to the app's code signature.
A build without a signing certificate is identified only by a hash of its binary. Every
update or rebuild changes that hash, so macOS treats the new version as a different app.
The switch in System Settings stays on, but it no longer applies, and commands run in
HyprSpace's terminal (for example `screencapture`) are refused.

To fix it on your Mac:

1. Open System Settings → Privacy & Security → the permission, for example
   **Screen & System Audio Recording**.
2. Select **HyprSpace** and click the minus button to remove it.
3. Click the plus button and add `/Applications/HyprSpace.app` again. If macOS asks to quit and
   reopen HyprSpace, do it.

That lasts until the next update. To keep permissions across updates, the app has to be
signed with the same certificate every time:

- **Local builds:** sign with a code-signing identity from your keychain, for example
  `APPLE_SIGNING_IDENTITY="Your Certificate" npm run tauri build -- --bundles dmg`.
- **Releases:** add the `APPLE_CERTIFICATE` (a base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`
  and `APPLE_SIGNING_IDENTITY` secrets and the release workflow signs the Mac app. Use a
  Developer ID Application certificate. `codesign` refuses a self-signed certificate on any
  machine that doesn't trust it, and a CI runner doesn't.

## Notes

- Ctrl+Click to open links works as **Cmd+Click** on macOS.
- If `npm run tauri build` complains it can't find a target, make sure you included
  `-- --bundles dmg` (the repo's default bundle target is the Windows installer).
