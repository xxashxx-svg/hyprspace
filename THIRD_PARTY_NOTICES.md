# Third-party notices

HyprSpace's own code is under the license in `LICENSE`. Code adapted from other projects keeps
its original license, listed here.

## zeron

Parts of the GPUI app are adapted from [zeron](https://github.com/zeronsh/zeron) at commit
`80b946b`:

- `crates/ui/src/terminal/emulator.rs` (including its selection and scrollback calls) from
  `crates/ui/src/terminal/emulator.rs`
- `crates/ui/src/terminal/keys.rs`, `crates/ui/src/terminal/paint.rs` and `paste_bytes` in
  `crates/ui/src/terminal/clipboard.rs` from `crates/ui/src/terminal/view.rs`
- the selection gestures in `crates/ui/src/terminal/mouse.rs` (drag threshold, click count to
  word and line, anchoring at the press) follow `crates/ui/src/terminal/panel.rs`
- `crates/harness/src/claude/` (the headless `claude` invocation, stdin line shapes, steer
  priorities and the held turn end, tool decoding, inline images, routing subagent frames and
  settling background subagents by `task_notification`) follows
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
- `crates/syntax/src/lib.rs` (the grammar set and versions, the capture table, the Rust and
  Markdown query fixes, and the precedence between overlapping captures) follows
  `crates/syntax/src/lib.rs`
- `scripts/package-macos.sh` (bundle layout, signing, notarizing and stapling before the
  updater tarball is made) follows `scripts/package-macos.sh`, and `relaunch_after_exit` in
  `crates/update/src/lib.rs` follows the function of that name in `crates/update/src/lib.rs`

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

## Tauri (NSIS template)

The shortcut macros in `apps/hyprspace/package/windows/utils.nsh` (`SetLnkAppUserModelId`,
`UnpinShortcut`, `SetShortcutTarget`, `IsShortcutTarget`) and the registry layout and flags of
`installer.nsi` are adapted from the NSIS template of
[tauri-bundler](https://github.com/tauri-apps/tauri) 2.x (`bundle/windows/nsis/`), copyright
the Tauri Programme within The Commons Conservancy, licensed under MIT or Apache-2.0 at your
option. The MIT terms:

```
MIT License

Copyright (c) 2017 - Present Tauri Apps Contributors

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

## Lucide

The icons in `crates/ui/assets/icons/` are from [Lucide](https://lucide.dev) (lucide-react
1.21.0), drawn from the same set the Tauri app uses. (`crates/ui/assets/logo/` is HyprSpace's
own cube from the Tauri app's `Logo.tsx`, one face per file.)

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

## Fonts

`crates/ui/assets/fonts/` holds the two fonts the Tauri app bundles, converted from its woff2
files to TrueType (GPUI loads TrueType):

- DM Sans, Copyright 2014 The DM Sans Project Authors (https://github.com/googlefonts/dm-fonts).
  Cut into static Regular, Medium, SemiBold and Bold instances from the variable font in
  `@fontsource-variable/dm-sans` (latin subset).
- JetBrains Mono, Copyright 2020 The JetBrains Mono Project Authors
  (https://github.com/JetBrains/JetBrainsMono), as patched by
  [Nerd Fonts](https://github.com/ryanoasis/nerd-fonts) ("JetBrainsMono Nerd Font Mono"), from
  `src/assets/fonts/`: Regular, Bold, Italic and Bold Italic.

Both are licensed under the SIL Open Font License, Version 1.1:

```
-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

