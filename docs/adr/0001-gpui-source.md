# ADR 0001: GPUI from one upstream Zed commit

- Status: Accepted
- Date: 2026-10-02

## Context

The GPUI app needs `gpui`, `gpui_platform` and `gpui_tokio`. None of them has a usable crates.io
release, so they come from git. Two sources were on the table:

- Upstream Zed (`zed-industries/zed`), pinned to one commit.
- zeron's fork `zeronsh/zui`, which zeron pins at `667d0aa`. It adds bounded backdrop blur, edge
  fades, macOS glass fixes and a Windows backdrop renderer. It also strips the `ztracing` crates
  out of `sum_tree`, which zeron's history records as a GPL fix.

## Decision

Use upstream Zed at `20d29fc6bc2fc2b58d1fff8d8e0503b9ba7f41d8` (`main` on 2026-10-02) for all
three crates, set once in the root `Cargo.toml`.

- **License.** Every Zed crate this pulls in is Apache-2.0 at this commit, `ztracing` and `zlog`
  included. `cargo metadata` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`
  lists no GPL crate and no crate without a license. Zed's GPL crates (`editor`, `ui`, `theme`,
  `markdown`, `terminal_view`) stay out.
- **It works on Windows as is.** The spike built with no `[patch]` entries on cargo 1.98.1 (the
  same toolchain Zed pins at that commit) and rendered text, quads and a terminal grid through
  the DirectX backend on Windows 11.
- **Fewer moving parts.** Zed ships on Windows and macOS and fixes its own backends. zui's
  additions are visual effects we don't need, and a fork's rebases trail upstream fixes.

## Consequences

- Bumping GPUI means changing one rev in `Cargo.toml` and rechecking the license list above.
- Zed's own workspace `[patch.crates-io]` section does not apply to us. If a bump fails to
  build, copying the relevant patch line into our root `Cargo.toml` is the first thing to try.
- If we later want frosted glass or edge fades, zui is the place to look, and this ADR gets
  superseded rather than edited.
- macOS was not built in the spike. `gpui_platform`'s `font-kit` feature is on because zeron
  needs it there; the macOS CI check in phase 2 confirms it.
