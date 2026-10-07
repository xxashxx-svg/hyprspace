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
The app on screen keeps running until a build succeeds. Windows dev builds link with `rust-lld`
(`.cargo/config.toml`) and dependencies carry no debug info, which took a one-file change from
about 14 s to 7 s.

A plain `cargo run` shares `~/.hyprspace/native` with an installed copy. Set `HYPRSPACE_STATE_DIR`
to another folder to keep them apart.

Checks that should have no side effects:

- `HYPRSPACE_USAGE_FIXTURES` reads usage from files instead of the endpoints.
- `HYPRSPACE_OPEN_LOG` logs editor and Explorer launches instead of running them.
- `HYPRSPACE_DEBUG_HOOKS=1` logs Claude's hook payloads (they hold prompts).

## The check

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

CI runs it on Windows and macOS (`.github/workflows/check.yml`).

## Building packages

```bash
cargo build --release -p hyprspace    # target/release/hyprspace(.exe)
./scripts/package-windows.ps1         # target/package/HyprSpace_<version>_x64-setup.exe
bash scripts/package-macos.sh         # target/package: HyprSpace.app, its .app.tar.gz, a dmg
```

A Mac build without the Apple secrets is signed ad hoc and not notarized, so its first launch is
blocked: right-click the app, Open, then Open (once), or System Settings, Privacy & Security, Open
Anyway. A locally built app never updates itself.
