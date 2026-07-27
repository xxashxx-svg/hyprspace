# HyprSpace for Android

The companion app: your spaces, your agents, and a **live mirror of any terminal** — on your phone,
over your own wifi. Same neutral dark theme as the desktop.

It is a companion, not a second copy of the app. Agents keep running on your computer; the phone
watches them, types into them, and starts new ones.

## What you get

| | |
|---|---|
| **Spaces & panes** | Every space, with each pane's live agent state (working / needs you / done) and what it's doing right now. Panes waiting on you float to the top of the home screen. |
| **Terminal** | A real mirror — xterm.js rendering the actual PTY stream at the desktop's own cols/rows. Typing goes straight through as you type, like sitting at the machine; the key row covers `esc` `tab` `↑↓` `^C` and the rest a phone keyboard doesn't have. |
| **Launch** | Start a Claude / Codex / Gemini / shell pane in any space from your phone. |
| **Changes** | The repo's changed files and their diffs, plus commit (and push). |
| **Automations** | What's scheduled, what's running, and run/stop. |
| **Usage** | Your Claude rate-limit windows, mirrored from the desktop's meter. |

## How it connects

There's no cloud service and no account. The desktop app runs a small WebSocket server
(`src-tauri/src/bridge.rs`, port 6768 by default); the phone dials it directly with a pairing token.
Terminals only mirror while HyprSpace is open on your computer.

The phone keeps a **list of ways in** and tries them top-down, promoting whichever connects:

| | |
|---|---|
| **Local network** | Your LAN address. Instant, and what you want at home. |
| **Away** | Optional second route for when you're not on that network. |

A dead address doesn't refuse — it hangs until the OS gives up — so each one gets 6 seconds before
the next is tried. Walking out of the house costs one failed attempt, then it's on the away route
until you're back.

For **Away**, use a VPN: install Tailscale on the desktop and the phone, and pair with the
`100.x.x.x` address it gives the desktop (Settings → Mobile lists every address, so it shows up in
the dropdown). Traffic is encrypted and authenticated by the VPN, and it needs no open ports. A
tunnel's public `wss://` URL (Cloudflare Tunnel, ngrok) works too.

**Don't just forward the port.** The bridge speaks plain `ws://` — fine on your own wifi, but over
the open internet it would send the pairing code and everything your agents print in the clear.

```
 phone (this app)  ── ws://<your-pc>:6768 ──  HyprSpace desktop
   src/rpc.ts                                   bridge.rs  ←→ PtyManager (live PTY bytes)
                                                    ↕
                                                src/mobileBridge.ts  (state mirror + actions)
```

The desktop pushes a state snapshot on every change, so the phone's lists update the moment the
desktop's do. Anything the phone asks for that needs the app itself (launch a pane, read a diff) is a
request the desktop's frontend answers. Terminal input goes straight to the PTY.

`PROTOCOL` in `src/rpc.ts` and `bridge.rs` must match — on a mismatch the app says which side is
older instead of half-working.

## Pairing

1. Desktop → **Settings → Mobile** → turn on **Sync to your phone**.
2. In this app: **Pair with desktop** → scan the QR.

Both devices need to be on the same wifi. If the camera isn't an option, the same screen takes the
address and code by hand.

## Running it

```bash
cd mobile
npm install
npm start            # then scan with Expo Go, or press `a` for a connected device/emulator
npm run typecheck
```

Because the app talks to a device on your LAN, **Expo Go on a real phone is the useful way to test**
— a desktop emulator can reach your machine, but you won't be testing the thing you actually built.

### Releasing

The app has **its own version line** — `deploy.ps1` doesn't touch it, because it ships as a GitHub
release asset rather than through the Tauri updater. See
[docs/VERSIONING.md](../docs/VERSIONING.md#the-android-app-mobile).

```bash
npm run version -- patch|minor|major     # bumps expo.version + android.versionCode together
npm run apk                              # → build/HyprSpace-<version>.apk
```

Then either attach that APK to the release by hand, or let CI do it:

```bash
gh workflow run release.yml -f tag=v0.14.1 -f android=true
```

The `android` job in `.github/workflows/release.yml` builds it and uploads two names — the versioned
one, and a stable `HyprSpace-android.apk` that the desktop's "Get the Android app" link points at, so
that link never goes stale.

**Before the first real release**, set two repo secrets or the CI build comes out debug-signed and
can never update an install:

| Secret | How |
|---|---|
| `ANDROID_KEYSTORE_BASE64` | `base64 -w0 ~/.hyprspace-signing/hyprspace-mobile.jks` |
| `ANDROID_KEYSTORE_PASSWORD` | the `HYPRSPACE_STORE_PASSWORD` line in `~/.gradle/gradle.properties` |

And **back that keystore up somewhere off this machine.** Android identifies an app by its signature:
once a version is out there, only that exact key can ship an update people can install over it. Lose
it and the only fix is a new package name and everyone reinstalling from scratch.

### Building an APK

Locally, with Gradle — no Expo account, no cloud queue:

```bash
npm run keystore     # once, ever: creates the release signing key
npm run apk          # → build/HyprSpace-<version>.apk
adb install -r build/HyprSpace-0.1.0.apk
```

You need **Android Studio** installed (for the SDK, and it bundles the JDK) and `ANDROID_HOME`
pointing at the SDK. The script finds the JDK itself, regenerates `android/` when the config has
changed, and copies the APK out. `npm run apk:debug` is the faster unminified build.

The first build downloads Gradle and the Android dependencies, so it takes a while; later ones are
minutes.

**Signing.** `npm run keystore` writes the key to `~/.hyprspace-signing/hyprspace-mobile.jks` and its
passwords to `~/.gradle/gradle.properties` — outside the repo, so `expo prebuild --clean` can't wipe
them and git can't leak them. **Back that keystore up**: Android identifies an app by its signature,
so losing it means never being able to ship an update that installs over what people already have.
Without it the build still works — it just falls back to debug signing, which is fine for sideloading.
The wiring lives in `plugins/withReleaseSigning.js` (a config plugin, because `android/` is generated).

EAS still works if you want it (`eas.json` is there — `npx eas build -p android --profile preview`),
but nothing depends on it.

## Layout

```
app/                 expo-router screens
  _layout.tsx        stack, theme, connection lifecycle, the offline banner
  index.tsx          home — spaces, and panes waiting on you
  pair.tsx           QR scan / manual pairing
  space/[id].tsx     one space: panes, launch, repo
  term/[id].tsx      the terminal mirror + composer + key row
  git/[id].tsx       changed files, diffs, commit
  automations.tsx    run / stop
  usage.tsx          rate-limit windows
  settings.tsx       connection, device name, unpair
src/
  theme.ts           the desktop's design tokens, ported (see src/styles/tokens.css)
  rpc.ts             the WebSocket client — state, pane streams, request/response
  store.ts           connection settings (persisted) + the state mirror
  ui.tsx             Screen / Card / Row / Btn / Label — the shared look
  PaneRow.tsx        a pane in a list, with its status dot
  terminal/termHtml.ts   GENERATED — xterm.js inlined into one HTML string
plugins/
  withReleaseSigning.js  makes release builds use your keystore (android/ is generated, so this
                         has to live here to survive `expo prebuild --clean`)
scripts/
  build-apk.mjs      local Gradle APK build — finds the JDK, prebuilds if stale, copies the output
  make-keystore.mjs  one-time release signing key, written outside the repo
  build-term.mjs     regenerates termHtml.ts from the desktop's @xterm/xterm
  build-icons.mjs    regenerates every icon from the HyprSpace mark
```

## Notes for whoever works on this next

- **Fonts are the platform's own** (`sans-serif` / `monospace`). Nothing is bundled, so nothing can
  fail to load and the APK stays small. The theme is otherwise the desktop's, token for token.
- **`termHtml.ts` is generated and committed.** It's xterm.js + its CSS baked into one string, so the
  WebView needs no network and no native asset plumbing. Re-run `npm run term` after the desktop
  upgrades `@xterm/xterm`, so both halves render with the same emulator. (`postinstall` does this
  automatically when the desktop's `node_modules` is present.)
- **The mirror renders at the desktop's cols/rows** and is scaled to fit, rather than reflowing. An
  agent TUI is laid out for the width it was given; re-wrapping it would just look broken. The `fit`
  button in the terminal header steps through zoom levels.
- **The phone never resizes the PTY.** Doing so would reflow the desktop's own view out from under
  whoever's sitting at it.
- **Icons come from the same mark as the desktop** (`src/components/Logo.tsx`) — regenerate with
  `npm run icons` rather than editing the PNGs.
- The app is versioned independently of the desktop (`app.json`), since it ships through a different
  channel. `deploy.ps1` doesn't touch it.
