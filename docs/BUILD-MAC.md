# Building HyprSpace on macOS

Release builds come from CI (`.github/workflows/release.yml`), so you only need this to build the
app yourself. It takes about 15 minutes the first time, mostly compiling GPUI. Apple silicon only:
the app ships for `darwin-aarch64`.

## 1. Install the prerequisites

```bash
# Xcode from the App Store, then its command-line tools and the Metal toolchain GPUI compiles
# its shaders with
xcode-select --install
xcodebuild -downloadComponent MetalToolchain

# Rust (accept the defaults, then restart your terminal), then the toolchain CI uses
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.98.1
```

## 2. Get the code and run it

```bash
git clone https://github.com/xxashxx-svg/hyprspace.git
cd hyprspace
cargo run -p hyprspace
```

## 3. Build the app bundle

```bash
bash scripts/package-macos.sh
```

It writes three things to `target/package`:

- `HyprSpace.app`, the bundle
- `HyprSpace.app.tar.gz`, what the updater installs
- `HyprSpace_<version>_aarch64.dmg`, the download with the drag-to-Applications layout

Without the Apple signing secrets the script signs the app ad hoc, which Apple silicon needs to
launch it at all. A locally built app is not notarized and never updates itself: rebuild from a
newer checkout to get changes.

## 4. Install and run

- Open the dmg and drag **HyprSpace** into Applications.
- The first launch is blocked because the app isn't notarized. **Right-click the app, then Open,
  then Open** (only needed once), or use System Settings, Privacy & Security, **Open Anyway**.

## Notes

- Shortcuts that use Ctrl on Windows use Cmd on macOS: Cmd+click a link or path, Cmd+K for the
  palette.
