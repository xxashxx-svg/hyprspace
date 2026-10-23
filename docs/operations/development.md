# Development

## Setup

- **Rust 1.98.1** through [rustup](https://rustup.rs) (`rustup toolchain install 1.98.1`), the
  toolchain CI uses.
- **Windows:** the MSVC C++ build tools ("Desktop development with C++" in the Visual Studio Build
  Tools). [NSIS 3](https://nsis.sourceforge.io) only to build the installer.
- **macOS (Apple silicon):** Xcode, its command-line tools (`xcode-select --install`) and the Metal
  toolchain GPUI compiles its shaders with (`xcodebuild -downloadComponent MetalToolchain`).

The first build compiles GPUI and takes a few minutes, about 15 on a Mac.

## Running it

```bash
./scripts/dev.ps1                     # rebuild on every save, swap the running app
./scripts/dev.ps1 -Fresh              # start the dev copy's state over
cargo run -p hyprspace -- <folder>    # one run, opened on a folder
```

`scripts/dev.ps1` keeps its own state in `~/.hyprspace/dev`, copied once from your real threads
with none of them open, so it never writes over the installed app's state or launches its agents.
The dev loop is Ash's, and Ash chats in the copy it runs. An agent checking its own change uses
[a test copy of its own](#a-test-copy-of-your-own).
The app on screen keeps running until a build succeeds. Windows dev builds link with `rust-lld`
(`.cargo/config.toml`) and dependencies carry no debug info, which took a one-file change from
about 14 s to 7 s.

A plain `cargo run` shares `~/.hyprspace/native` with an installed copy. Set `HYPRSPACE_STATE_DIR`
to another folder to keep them apart.

Checks that should have no side effects:

- `HYPRSPACE_USAGE_FIXTURES` reads usage from files instead of the endpoints.
- `HYPRSPACE_OPEN_LOG` logs editor and Explorer launches instead of running them.
- `HYPRSPACE_DEBUG_HOOKS=1` logs Claude's hook payloads (they hold prompts).

## A test copy of your own

A copy of the app on state you wrote, with a fake agent, for seeing a change without touching
anyone's threads. On Windows, in PowerShell from the repo root. A shell that keeps no variables
between calls runs steps 1 to 3 as one script and reads the PID back from `$dir\pid.txt` later.

1. Build the app and the fake agent, and copy the app out of `target/`. A running exe is locked,
   and the next build writes there.

   ```powershell
   $dir = "$env:TEMP\hyprspace-test"
   cargo build -p hyprspace -p hyprspace-harness
   New-Item -ItemType Directory -Force "$dir\bin", "$dir\state\journals" | Out-Null
   Copy-Item target\debug\hyprspace.exe $dir
   Copy-Item target\debug\fake-cli.exe "$dir\bin\claude.exe"
   ```

2. Write `$dir\state\state.json`. Every field has a default except a launch's `agent`, `cwd` and
   `permission`. Leave out `resume`, so no thread can reopen a real conversation:

   ```json
   {"spaces":[{"id":1,"name":"test","cwd":"C:\\path\\to\\folder","threads":[
     {"id":2,"title":"Test","kind":{"type":"structured","launch":
       {"agent":"claude","cwd":"C:\\path\\to\\folder","permission":"ask"}}}]}],
    "nextId":3,"active":2,"introSeen":true,"composer":{"agent":"claude"}}
   ```

   Spaces and threads share one id counter, and `active` is the thread that opens.
   `$dir\state\journals\thread-2.jsonl` holds what the thread replays, one `Entry`
   (`crates/proto/src/state.rs`) per line. Write as much as the test needs: a scrolling bug wants
   250 prompts, not 3.

   ```json
   {"type":"prompt","prompt":{"text":"Hello","images":[]}}
   {"type":"run","event":{"type":"text","text":"A reply."}}
   {"type":"run","event":{"type":"finished","status":"done","ms":0,"text":"","error":null}}
   ```

3. Start it on that state, with usage and opens kept local and the fake agent first on its PATH:

   ```powershell
   $env:HYPRSPACE_STATE_DIR = "$dir\state"; $env:HYPRSPACE_OPEN_LOG = "$dir\open.log"
   $env:HYPRSPACE_USAGE_FIXTURES = "$dir\usage"; $env:PATH = "$dir\bin;$env:PATH"
   $app = Start-Process "$dir\hyprspace.exe" -PassThru
   $app.Id | Out-File "$dir\pid.txt" -Encoding ascii
   ```

   The fake agent plays what the prompt's text names (`crates/harness/fixtures/fake_cli/claude.rs`).
   `wedge` starts a run that never ends, so what you send next queues behind it. Check that the
   composer shows Claude and Structured before you send: a terminal thread's shell can still find
   the real `claude`.

4. Drive it by `$app.Id`. Find its window, post `WM_MOUSEMOVE`, `WM_LBUTTONDOWN`, `WM_LBUTTONUP`,
   `WM_CHAR` and `WM_KEYDOWN` to it in client pixels, and capture it with `PrintWindow` (flag 3)
   after `SetProcessDPIAware`. A chord with Ctrl or Alt needs the modifier held through
   `AttachThreadInput` and `SetKeyboardState`. Posted clicks don't reach the sidebar, so pick the
   thread that opens with `active` instead.

5. Close it with `$app.CloseMainWindow()` and wait for it to exit, so it ends its terminals. Force
   it by that PID only if it hangs, then check for a leftover `OpenConsole.exe` whose parent was
   that PID.

## The check

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

CI runs it on Windows and macOS (`.github/workflows/check.yml`).

## Measuring

Before and after work meant to make the app faster, run the measurements in a release build:

```bash
cargo test --release -p hyprspace-ui perf_ -- --ignored --nocapture
```

Each prints what it measured: terminal parsing and grid snapshots, a long reply parsed whole and
while it streams, and a long thread replayed. Each also fails past a generous budget. They are
skipped in the check, because timings on shared CI machines and in debug builds don't mean much.

## Building packages

```bash
cargo build --release -p hyprspace    # target/release/hyprspace(.exe)
./scripts/package-windows.ps1         # target/package/HyprSpace_<version>_x64-setup.exe
bash scripts/package-macos.sh         # target/package: HyprSpace.app, its .app.tar.gz, a dmg
```

A Mac build without the Apple secrets is not notarized, so its first launch is blocked: right-click
the app, Open, then Open (once), or System Settings, Privacy & Security, Open Anyway. A local build
without `MACOS_SIGNING_CERTIFICATE` is signed ad hoc, so macOS treats each one as a new app and asks
for permissions again. A locally built app never updates itself.

## The Android app

`mobile/` is a Gradle project: Kotlin, Compose, AGP 9. It needs JDK 17 and the Android SDK
(`ANDROID_HOME`, or `sdk.dir` in `mobile/local.properties`) with platform 37.

```bash
cd mobile
./gradlew assembleDebug          # app/build/outputs/apk/debug, installs as "com.hyprspace.android.dev"
./gradlew testDebugUnitTest      # the wire fixtures, the transcript, terminal frames, pairing links
./gradlew assembleRelease        # shrunk; signed with the debug key unless the release variables are set
```

To try it against a dev build, set `HYPRSPACE_PHONE_BIND=127.0.0.1` for the desktop: the bridge
stays on loopback, so no firewall asks about it, and an emulator reaches it at `10.0.2.2`. Pair
by opening the QR code's link with the host swapped, `adb shell am start -d
"hyprspace://pair?...&h=10.0.2.2&..."`, or type `10.0.2.2:47821` and the code by hand.

A change to `crates/proto/src/phone.rs` or the types it carries rewrites the fixtures the app's
tests read: `HYPRSPACE_WRITE_FIXTURES=1 cargo test -p hyprspace-proto phone`.
