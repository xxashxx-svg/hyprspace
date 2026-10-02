# Third-party notices

HyprSpace's own code is under the license in `LICENSE`. Code adapted from other projects keeps
its original license, listed here.

## zeron

Parts of the GPUI app are adapted from [zeron](https://github.com/zeronsh/zeron) at commit
`80b946b`:

- `crates/ui/src/terminal/emulator.rs` from `crates/ui/src/terminal/emulator.rs`
- `crates/ui/src/terminal/keys.rs` and `crates/ui/src/terminal/paint.rs` from
  `crates/ui/src/terminal/view.rs`
- `crates/harness/src/claude/` (the headless `claude` invocation, stdin line shapes, steer
  priorities and the held turn end, tool decoding, inline images) follows
  `crates/harness/src/claude/mod.rs`, `wire.rs` and `normalize.rs`
- `crates/harness/src/codex/` (the app-server handshake, thread and turn requests, steering with
  its late-steer fallback, approvals, item mapping) and `codex/rpc.rs` follow
  `crates/harness/src/codex/mod.rs`, `normalize.rs` and `crates/harness/src/jsonrpc.rs`
- the stderr tail in `crates/harness/src/spawn.rs` follows `StderrTail` in
  `crates/harness/src/lib.rs`
- the fake CLI in `crates/harness/fixtures/fake_cli/` follows the scenarios in
  `crates/harness/tests/fixtures/fake-claude.sh` and `fake-codex.sh`
- the crate split (`proto`, `harness`, `engine`, `ui`, `theme`) and `docs/CONTEXT.md` follow
  zeron's layout and its `CONTEXT.md`
- `crates/ui/src/markdown/` follows the shape of `crates/ui/src/markdown/` and
  `crates/markdown/src/parser.rs` (a pulldown-cmark block tree with flattened inline runs, drawn
  as styled text)
- the sidebar's drag-to-resize edge in `crates/ui/src/sidebar/mod.rs` follows `resize_handle`
  and `on_sidebar_drag` in `crates/ui/src/shell.rs`

```
MIT License

Copyright (c) 2026 Wing

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Zed (GPUI examples)

`crates/ui/src/input/` (the text box's selection model, UTF-16 bridging and input handler) is
adapted from `crates/gpui/examples/input.rs` in [Zed](https://github.com/zed-industries/zed) at
commit `20d29fc6bc2fc2b58d1fff8d8e0503b9ba7f41d8`, licensed under the Apache License 2.0
(https://www.apache.org/licenses/LICENSE-2.0). Changes: wrapping and several lines, vertical and
word moves, and events for submit, cancel and pasted images.

## Lucide

The icons in `crates/ui/assets/icons/` are from [Lucide](https://lucide.dev) (lucide-react
1.21.0), drawn from the same set the Tauri app uses.

```
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

The agent marks in `crates/ui/assets/brand/` are the Claude and OpenAI logos the Tauri app
already ships in `src/assets/brand/`, used to name the agent a thread runs. They are trademarks
of their owners.
