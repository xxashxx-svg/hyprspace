# HyprSpace

HyprSpace is a native desktop workspace for coding agents. Each folder you work in is a space,
each conversation in it is a thread, and each thread runs Claude Code or Codex as a live transcript
or in a real terminal. It's written in Rust on [GPUI](https://github.com/zed-industries/zed), Zed's
GPU UI framework.

It works with your own subscriptions. HyprSpace starts the `claude` and `codex` CLIs you already
have installed and logged in, and never touches your credentials.

## Installation

> [!WARNING]
> Install and log in to at least one agent first:
>
> - Claude: install [Claude Code](https://claude.com/product/claude-code) and run `claude` once to log in
> - Codex: install [Codex CLI](https://developers.openai.com/codex/cli) and run `codex login`
>
> A plain shell works without either.

On macOS:

```bash
curl -fsSL https://hyprspace.dev/install.sh | sh
```

On Windows, in PowerShell:

```powershell
irm https://hyprspace.dev/install.ps1 | iex
```

Or download the `.dmg` or `.exe` from [GitHub Releases](https://github.com/xxashxx-svg/hyprspace/releases).
HyprSpace runs on Windows and on macOS with Apple silicon, and an installed copy updates itself.

## Some notes

HyprSpace is early. Expect bugs.

## Building from source

You need Rust 1.98.1 (`rustup toolchain install 1.98.1`). On Windows, add the MSVC C++ build tools.
On macOS, add Xcode and its Metal toolchain ([docs/operations/development.md](./docs/operations/development.md)).

```bash
git clone https://github.com/xxashxx-svg/hyprspace.git
cd hyprspace
cargo run -p hyprspace
```

Read [AGENTS.md](./AGENTS.md) before writing code. It's the project guide, for people and agents
alike. [docs/](./docs/README.md) goes deeper.

## Contributing

Issues and PRs are welcome. Read [CONTRIBUTING.md](./CONTRIBUTING.md) first. Found a security
issue? See [SECURITY.md](./SECURITY.md) instead of opening an issue.

## License

MIT. See [LICENSE](./LICENSE). Code adapted from other projects is credited in
[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md). The File Explorer icon in the Open menu is by
[Icons8](https://icons8.com), used under their free license.
