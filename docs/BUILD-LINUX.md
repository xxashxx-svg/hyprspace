# Building the Linux app

HyprSpace ships two Linux bundles: an **AppImage** (the only Linux format Tauri's updater can
self-update) and a **.deb** for people who'd rather use their package manager. Both are built by the
`linux` job in [`.github/workflows/release.yml`](../.github/workflows/release.yml) — you don't need a
Linux machine to cut a release.

## What's different on Linux

| | Windows | macOS | Linux |
|---|---|---|---|
| Webview | WebView2 (Chromium) | WebKit | **WebKitGTK** (`webkit2gtk-4.1`) |
| Shell in a pane | `powershell.exe` | `$SHELL` | `$SHELL` → `/bin/bash` |
| Bundle | NSIS installer | .app / .dmg | AppImage + .deb |
| Auto-update | yes | yes | **AppImage only** |
| Built by | `deploy.ps1` (local) | CI | CI |

Two consequences worth knowing before you touch this:

- **`additionalBrowserArgs` in `tauri.conf.json` does nothing here.** Those flags
  (`--disable-background-timer-throttling` and friends) are WebView2-only. If background panes ever
  misbehave on Linux, that config is not the lever — it's silently ignored.
- **A `.deb` cannot self-update.** Tauri's updater only supports AppImage on Linux. The `.deb` is a
  convenience for people who prefer apt; those users update by reinstalling. Don't add a
  `linux-x86_64` manifest entry pointing at a `.deb` — the client can't apply it.

## Building locally

You need a Linux box, a VM, or WSL2. **WSL2 can compile but not display the app** unless you're on
Windows 11 (WSLg) — on Windows 10 there's no GUI without an X server, and WebKitGTK under a
software X server tells you nothing useful about rendering performance.

```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev patchelf \
  libxdo-dev libssl-dev libayatana-appindicator3-dev \
  build-essential curl wget file

npm install
npm run tauri dev                                  # HMR, same as any platform
npm run tauri build -- --bundles appimage,deb      # config's default target is nsis, so override
```

Bundles land in `src-tauri/target/release/bundle/{appimage,deb}/`.

`--bundles` is required because `bundle.targets` in `tauri.conf.json` is `["nsis"]` — that default
exists so a bare `npm run tauri build` on Windows does the right thing. CI passes the same override.

## The GPU renderer

Terminals use xterm.js's WebGL addon, and WebGL on WebKitGTK varies by driver and compositor.
This is already handled rather than assumed: `attachGpuRenderer()` in
[`src/terminal/createTerminal.ts`](../src/terminal/createTerminal.ts) try/catches addon construction
and registers an `onContextLoss` handler, so a failure falls back to xterm's DOM renderer instead of
breaking the pane. There's also a user-facing **GPU rendering** toggle in Settings → Terminal.

If Linux users report sluggish terminals, check whether WebGL attached at all before optimising
anything else — the DOM renderer is the likely culprit, and the toggle is the first thing to try.

Note that VMs give you software rendering (llvmpipe), so a VM cannot answer performance questions
about the GPU path. Only real hardware can.

## Frameless window

`decorations: false` means the app draws its own titlebar on Linux exactly as it does on Windows.
The edge/corner resize grips are the `RESIZE` handles in [`src/App.tsx`](../src/App.tsx) (rendered
for every non-mac platform) calling Tauri's `startResizeDragging`; dragging and double-click-maximise
live in [`Titlebar.tsx`](../src/components/Titlebar.tsx). Window snapping and drop shadows are up to
the desktop environment and will differ between GNOME, KDE and the tiling WMs.

## Release flow

`deploy.ps1` publishes the Windows half first, then triggers this workflow. The jobs run in sequence:

```
deploy.ps1 (windows-x86_64)  →  macos job (darwin-aarch64)  →  linux job (linux-x86_64)
```

Each one downloads the release's `latest.json`, adds **only its own** platform entry via
[`scripts/ci-build-latest.mjs`](../scripts/ci-build-latest.mjs), and re-uploads. The `linux` job
declares `needs: macos` for exactly this reason — running them concurrently would let one clobber
the other's entry. If you add a fourth target (say `darwin-x86_64`), add a row to `PLATFORMS` in
that script and chain the new job after `linux`.

The AppImage is also uploaded under the stable name `HyprSpace-linux-x86_64.AppImage` so the
website's download link always resolves to the newest build.
