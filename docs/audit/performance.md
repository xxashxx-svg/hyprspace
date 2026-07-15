# Performance audit — 2026-07-15

Full performance pass over the app: the terminal/PTY data path, React rendering + Zustand
subscription patterns, the Rust backend commands, the long-running engines (chat, Loops, git
polling, settings sync), and build/bundle/CSS. Complements [`security.md`](./security.md) and
[`bugs.md`](./bugs.md) — this file is only about speed, jank, CPU/battery, memory, and size.

Every finding below was verified against the actual code (file:line quoted).

## Status — fixed 2026-07-15 (same branch)

Everything high/medium and most lows were fixed the same day, verified with `cargo check`,
`npx tsc --noEmit`, and a production `vite build`:

- **Fixed:** P1 (xterm flow control via new `pause_pty`/`resume_pty` + pending-byte accounting,
  1MB high / 128KB low water), P2–P4 + P16 (every remaining sync command is now
  `async` + `spawn_blocking`), P3/P9 (kills happen after the map guard drops; chat stdin sits
  behind its own lock), P5 (loops persist debounced 300ms + flush-on-hide), P6 (run streams
  batch ~100ms into plural store ops; Rail/Titlebar/LoopsPage select primitives; EventRow
  memoized), P7 (13 lazy surfaces, vendor manualChunks, per-language CodeMirror imports —
  entry chunk 215KB/71KB gzip), P8 (in-flight guard + visibility/view gate + payload compare;
  DiffView memo + 2000-line cap; lazy CodeEditor), P10 (async resize + no-op skip + rAF'd zoom),
  P11 (per-session Rail rows on primitive selectors; no rail-wide interval left), P12 (pj
  buffered on the 55ms throttle, tail-swap patchMsg, in-memory cap, memoized Message),
  P13 (streamed reads + tail-read), P14 (services batched ~30ms into joined sends),
  P15 (woff2, ~9.9MB → ~4.2MB), and lows L1–L8, L10–L12, L14, L15, most of L16.
- **Left open (deliberate):** per-line channel sends in agent.rs/chat.rs (protocol-paced, a
  batching change would touch the stream parsers for little gain), the backend git
  status+branch fold-into-one-process idea in P8 (frontend gating removed ~all the waste),
  L9 (focus-only persist no-ops — the signature compare already prevents the disk write),
  L13 (rail width transition — visual-risk/benefit not worth it), the duplicate boot
  `loadState("settings")` (one redundant read), and supabase stays an eager client (but now
  in its own split chunk; swapping to `@supabase/auth-js` is a bigger auth refactor).
- Incidental: added the Linux keyring backend to Cargo.toml — the Linux build didn't compile
  at all before (keyring was only declared for Windows/macOS targets).

## Headline

The hot paths that were designed deliberately are in good shape: PTY output is byte-coalesced
(4 ms/16 KB) and crosses IPC as raw binary, panes are memoized and never remount on space
switches, chat text deltas are batched at 55 ms, workspace/settings saves are debounced with
no-op signature checks, and every interval/listener audited has correct cleanup.

The problems cluster in four places:

1. **Sync Tauri commands still doing blocking work on the UI thread** — persistence (with a real
   `fsync`), process kill via `taskkill`, PTY create/resize, keychain reads. The repo's own rule
   ("anything filesystem-heavy is `async fn` + `spawn_blocking`") is applied to half the commands.
2. **No flow control across IPC into xterm** — a flooding child can balloon memory and saturate
   the UI thread; every layer below it has backpressure, the last hop doesn't.
3. **Store fan-out** — chatty producers (loop output lines, pane activity) write to store keys
   that app-chrome (Rail, Titlebar, LoopsPage) subscribes to wholesale, so single output lines
   re-render the whole sidebar.
4. **Zero code-splitting** — CodeMirror + 7 language packs, supabase-js, and every dialog/page
   load and parse before the sign-in screen can paint.

---

## High severity

### P1 — No end-to-end flow control: xterm write backpressure is ignored
`src/components/TerminalPane.tsx:256` · `src-tauri/src/pty.rs:166`

PTY output is pushed with a fire-and-forget `term.write(bytes)` and the Rust `Channel::send`
never blocks. The bounded `sync_channel(256)` in `pty.rs` only throttles the reader thread vs.
the coalescer — once a chunk is sent, it queues unboundedly in the Tauri IPC queue and then in
xterm's internal write buffer. A child that floods (`cat` of a big file, a verbose build, a
runaway agent) pumps ~16 KB every 4 ms while xterm parses a few MB/s on the main thread: the
write buffer balloons to hundreds of MB and the UI saturates. This is the one genuinely
unbounded-memory path in the app.

**Fix:** the standard xterm flow-control pattern (what VS Code does): count outstanding bytes
via `term.write(bytes, cb)`; past a high-water mark, tell the backend to pause the PTY reader
(the kernel PTY buffer then backpressures the child), resume at a low-water mark.

### P2 — State persistence is a sync command doing `fsync` on the UI thread
`src-tauri/src/lib.rs:266` · `src-tauri/src/persist.rs:44`

`save_state` / `load_state` / `backup_state` are plain (non-async) commands, so every save runs
`File::create` + `write_all` + `sync_all()` + `rename` on the main thread. `sync_all` is a real
disk flush — 1–50 ms, worse on HDD/AV-scanned Windows. Callers hit it often: workspace saves
(debounced only 300 ms during layout churn), the loops blob, and the chat blob — up to 30
threads × 200 msgs × 4 KB tool results, potentially several MB of JSON — written after every
turn. Each one is UI-thread jank. `load_state` also reads the whole blob synchronously at boot.

**Fix:** make all three `async fn` + `spawn_blocking` (the `Store` is mutex-guarded already —
mechanical change). Consider dropping `sync_all` for low-value blobs.

### P3 — Stop/start commands run `taskkill` (blocking spawn+wait) on the UI thread, with the manager lock held
`src-tauri/src/lib.rs:59-118` · `src-tauri/src/chat.rs:62` · `src-tauri/src/agent.rs` · `src-tauri/src/services.rs`

`chat_stop` / `service_stop` / `agent_stop` / the `*_start` trio are sync commands. On Windows,
kill = spawn `taskkill /T /F` via `.output()` (blocking, easily 50–300 ms for a process tree)
plus `child.wait()` — all on the UI thread. Worse, `reap()` uses
`if let Some(proc) = self.procs().remove(id) { kill_proc(proc) }`, and under edition 2021 the
`MutexGuard` temporary lives for the whole `if let` body — the kill runs **with the process map
locked**. `agent_start` additionally does OS keychain reads and a `cmd /c` spawn on the UI
thread once per loop iteration.

**Fix:** `async` + `spawn_blocking` like `write_pty`/`chat_turn` already are; remove from the
map inside a short lock, kill after the guard drops.

### P4 — `create_pty` is sync: ConPTY open + shell spawn on the UI thread, ×N on launcher fan-out
`src-tauri/src/lib.rs:23-38`

ConPTY creation + `CreateProcess` of `powershell.exe` are blocking syscalls that routinely take
tens of ms each. The launcher mounts N `TerminalPane`s in one commit, each calling `createPty`
— N sequential blocking spawns serialize on the UI thread and freeze the window during exactly
the moment the grid is animating in. `write_pty` next to it already does the right thing.

**Fix:** `async fn` + `spawn_blocking` (PtyManager is `Clone`, the Channel is `Send`).

### P5 — Loops editor persists the entire loops store to disk on every keystroke
`src/components/LoopsManager.tsx:362,424,…` · `src/stores/loops.ts:188-191`

Every `onChange` of the name/prompt/cron/sentinel/model fields calls `update()` → `upsert()`,
and `upsert` unconditionally does `persist()` — a full-store `JSON.stringify` plus a crash-safe
temp-file+fsync+rename write (see P2) per keypress. Typing a 200-char prompt = 200 disk flushes
on the UI thread. `settingsSync.ts` debounces for exactly this reason; this path forgot to.

**Fix:** debounce `persist()` ~300 ms, or commit text fields on blur (the `NumField` in the same
file already uses the blur pattern).

### P6 — Loop run streaming: two store writes per output line fan out to the whole app chrome
`src/lib/loops.ts:385-417,523` · `src/stores/loops.ts:208-221` · `src/components/Rail.tsx:73` · `src/components/Titlebar.tsx:254` · `src/components/LoopsPage.tsx:85` · `src/components/LoopRunView.tsx:159`

Every stdout line of a running loop calls `pushEvent` **and** `appendLog`, each cloning arrays
up to 1500/4000 entries (`[...cur.logs, line]` — O(n) per line, O(n²) per run) and minting a new
`runs` object. Rail, Titlebar, and LoopsPage all subscribe to `s.runs` by identity, so **every
output line of any loop re-renders the entire sidebar and titlebar twice** — even when the
Loops page isn't open, even minimized. With the Runs view open, `LoopRunView` re-creates up to
1500 unmemoized `EventRow`s per line on top.

**Fix:** batch line appends on a ~100 ms flush; have chrome select derived primitives (active
count) instead of the `runs` object; `React.memo(EventRow)`.

### P7 — Zero code-splitting; CodeMirror + 7 language packs and supabase-js parse before first paint
`src/App.tsx:12-32` · `src/components/CodeEditor.tsx:2-13` · `src/lib/supabase.ts:21` · `vite.config.ts`

There is not a single `import()` / `React.lazy` / `Suspense` in `src/`, and no `manualChunks`.
The entry chunk contains Settings, Onboarding, LoopsPage, LaunchWorkspace, ReviewDock, every
dialog, xterm + 5 addons, `basicSetup` CodeMirror plus `lang-javascript/json/css/html/markdown/
python/rust` (~1 MB+ minified, for an editor tab most sessions never open, of which one language
at most is needed per file), and an eagerly-constructed supabase client whose realtime/postgrest/
storage sub-clients are dead code. All of it must fetch+parse+compile before `AuthGate` paints.

**Fix:** `React.lazy` the conditional surfaces (Settings, Onboarding, LoopsPage, LaunchWorkspace,
CodeEditor inside ReviewDock, dialogs); per-extension dynamic import of CodeMirror language
packs; lazy-init supabase (or `@supabase/auth-js` only); add vendor `manualChunks`.

---

## Medium severity

### P8 — ReviewDock git poll: every 4 s, no in-flight guard, keeps running while hidden, re-renders unconditionally
`src/components/ReviewDock.tsx:64-76,247,24-43` · `src-tauri/src/devtools/git.rs:27,424`

The 4 s tick fires `gitChanges` + `gitBranchInfo` (~7 git process spawns per tick on the Rust
side — `rev-parse` ×3, `diff --numstat` ×2, `status`, `rev-list`) with: (a) no in-flight guard —
on a repo where `git status` takes >4 s, calls pile up; (b) no visibility gate — it polls while
minimized and also on the Loops/Launch views, because the early return is only
`view === "home"` while `App.tsx` hides the workspace view with `display:none`; (c) fresh
array/object identities every tick, so the panel re-renders even when nothing changed, and the
unmemoized `DiffView` re-splits the diff and rebuilds one `<div>` per line (thousands for a
lockfile diff) every 4 s while open.

**Fix:** in-flight boolean, gate on `view === "space" && !document.hidden`, compare payloads
before setState, `memo(DiffView)` + cap/virtualize lines. Backend: cache the is-repo check per
cwd and fold status+branch into one `git status --porcelain=v2 --branch` call.

### P9 — `ChatManager::turn` holds the process-map mutex across a blocking pipe write
`src-tauri/src/chat.rs:136-146`

`turn` keeps the map guard across `write_all`/`flush` to the child's stdin. The whole reason
`chat_turn` is routed through `spawn_blocking` is that a child that stops draining stdin blocks
the write — but when that happens the write blocks **holding the lock**, and `chat_stop` (sync,
UI thread, P3) then blocks on that same lock in `reap()`. The mitigation converts into an
indefinite UI freeze, and the wedged process can't be killed because killing needs the lock.

**Fix:** store stdin per-proc behind its own `Arc<Mutex<ChildStdin>>`; clone it out of the map,
drop the map guard, then write.

### P10 — `resize_pty` is sync and gets called even when dimensions didn't change; Ctrl+wheel zoom is unthrottled
`src-tauri/src/lib.rs:50` · `src/components/TerminalPane.tsx:300-330,335-341,454-467`

`ResizePseudoConsole` (cross-process ConPTY IPC) runs on the main thread, and the frontend calls
`resizePty` after every `fit()` even when `cols/rows` are unchanged — during a window drag with
a 6–12 pane grid that's potentially 100–200 main-thread round-trips/sec. Ctrl+wheel zoom writes
`fontSize` per wheel notch (~30–60 Hz) with no throttle; every mounted pane (including hidden
spaces) then re-measures fonts, rebuilds the WebGL glyph atlas, and fires another `resizePty`.

**Fix:** async + `spawn_blocking` the command; remember last-sent `(cols, rows)` and skip
no-ops; rAF/50 ms-coalesce the zoom handler.

### P11 — Rail re-renders once a second while expanded, plus on every pane's activity bump
`src/components/Rail.tsx:80-81,141-145` · `src/stores/activity.ts:24-32`

A 1 s `setInterval` force-renders the entire sidebar while any workspace is expanded (just to
decay "busy" dots), including when idle and minimized. Rail also subscribes to the whole
`lastOut`/`exited` maps, whose identity changes on every per-pane bump (throttled 500 ms per
pane) — with N busy panes the full Rail (filter chains, auto-animate lists) renders up to
2N times/sec, forever.

**Fix:** per-session leaf component subscribing to `s.lastOut[id]` primitives, own its dot decay
timer; gate the tick on visibility.

### P12 — Chat streaming costs (latent — ChatPanel is currently parked)
`src/stores/chat.ts:250-257,57-62` · `src/components/ChatPanel.tsx:241,385-388`

The home chat is unmounted today (`HomePage.tsx:151`), so these cost nothing yet — but they're
the first thing that will hurt when it returns:
- `input_json_delta` chunks bypass the 55 ms text throttle and call `patchMsg` per chunk — a
  large streamed `Write` tool input = hundreds of store sets/sec, each rebuilding
  `threads → messages → blocks`.
- `Message` isn't memoized, so every flush re-runs `react-markdown` + `remark-gfm` over **every**
  message in the thread — O(thread length) parser work per 55 ms tick.
- In-memory threads are unbounded (only the disk blob is capped at 30/200/4 KB), and `patchMsg`
  copies all threads plus the whole target message array per event, though the patched message
  is always the last one.

**Fix (when re-enabling):** buffer `partial_json` in the `Live` object and flush on the same
timer / block stop; `React.memo(Message)`; apply the disk caps in memory; tail-swap instead of
`messages.map`.

### P13 — Usage scan reads whole transcripts (up to 80 MB each, 160 files) into RAM
`src-tauri/src/devtools/usage.rs:273-303,385-400`

`sum_claude_tokens` does `read_to_string` per transcript with `MAX_FILE_BYTES = 80 MB`, and
`last_token_count` reads an entire Codex rollout to find the **last** matching line. Correctly
on `spawn_blocking` (no jank), but opening Settings → Usage can chew hundreds of MB of reads and
tens-of-MB string allocations.

**Fix:** stream with `BufReader::lines()`; tail-read the last ~64 KB for `last_token_count`;
lower the cap.

### P14 — Per-line Channel sends with no coalescing in the services/agent/chat readers
`src-tauri/src/services.rs:94-110` · `src-tauri/src/agent.rs:119-135` · `src-tauri/src/chat.rs:104-125`

Every stdout/stderr line is one `Channel::send` — one IPC message into the webview. A background
service running a dev server/build can emit thousands of lines/sec → thousands of webview hops/
sec. This is exactly the flood `pty.rs` coalesces to avoid; these paths have no batching.
Services is the worst (long-lived and chatty); chat/agent output is naturally paced.

**Fix:** buffer lines, flush on a ~16–33 ms timer or N-line batch as a JSON array per send.

### P15 — 9.5 MB of uncompressed Nerd Font TTFs bundled, plus overlapping mono families
`src/assets/fonts/` (4 × ~2.47 MB) · `src/styles/fonts.css` · `src/main.tsx:3-7`

Four raw TTFs ship in the installer (woff2 would cut ~60-70%); Italic/BoldItalic Nerd faces are
rarely hit by terminal output; Cascadia + JetBrains Mono fontsource + JetBrains Nerd overlap in
role. Costs download/disk and first-paint font parsing (`font-display: block` blanks text
briefly).

**Fix:** convert to woff2, lazy-declare or drop the italics, prune the unused mono family.

### P16 — Remaining sync commands doing OS round-trips on the UI thread
`src-tauri/src/lib.rs:123-144,196-199`

`secret_set/has/clear` hit the OS keychain (out-of-process RPC that can stall on slow credential
providers) synchronously — `secretHas` runs on Loops UI mount and before every loop run.
`claude_has_history` enumerates `~/.claude/projects/<enc>` (the directory the code itself
comments "can hold a LOT of transcripts") synchronously — currently no frontend call sites, but
a loaded footgun since its siblings got the async treatment.

**Fix:** `async fn` + `spawn_blocking`, same as the rest.

---

## Low severity

- **L1 — `write_pty` sends input as a JSON number array** (`src/api/index.ts:45`):
  `Array.from(data)` → `[104,101,…]` is ~4–5 bytes JSON per input byte; a 1 MB paste becomes a
  ~4–5 MB JSON string built on the UI thread. Use a raw invoke body or base64.
- **L2 — No `[profile.release]` in `src-tauri/Cargo.toml`**: cargo defaults (no LTO, 16 codegen
  units, `panic=unwind`, no strip) — larger auto-update download and slower code than the
  standard Tauri profile.
- **L3 — Window refocus rebuilds the texture atlas for every mounted pane**
  (`TerminalPane.tsx:344-363`): each pane — including `display:none` ones in inactive spaces —
  registers its own `focus`/`visibilitychange` handler; one alt-tab = atlas clear + full refresh
  × all mounted panes in the same frame. Skip when not active.
- **L4 — Log viewers re-render every line per incoming line** (`ServiceLogs.tsx:70`,
  `LoopsManager.tsx:725`): whole-buffer copy per append (caps 2000/4000), ANSI-strip regex runs
  at render time over all lines, index keys shift after cap splice. Clean at append time,
  memoize rows.
- **L5 — CommandPalette scans all terminal buffers per keystroke**
  (`CommandPalette.tsx:134-161`): `toLowerCase()` over every session's 24 KB buffer in the input
  render path (~500 KB string churn per keystroke at 20 panes). Debounce ~100–150 ms.
- **L6 — Interactive-loop marker polling** (`lib/loops.ts:726-748`): 2 `readFile` IPC calls per
  1.5 s per interactive loop for its whole (possibly hours-long) life, re-splitting the full
  marker file each tick. Back off to 3–5 s, track file length.
- **L7 — `nextCron` worst case walks 527k minutes on the UI thread** (`lib/cron.ts:81-97`): a
  never-matching expression (e.g. `0 0 31 2 *`) costs a 10–50 ms stall per re-arm. Skip whole
  days when day fields can't match.
- **L8 — EntitlementGate re-checks on every window focus, unthrottled**
  (`EntitlementGate.tsx:18`): once the backend is live, every alt-tab fires a network
  `functions.invoke`. Copy the Updater's 15-min focus throttle.
- **L9 — Focus clicks schedule no-op workspace persists** (`App.tsx:130-145`): `setFocused`
  triggers the debounced full-tree `JSON.stringify` just to discover via the signature compare
  that nothing changed. Ignore focus-only updates.
- **L10 — `auto_respond` scans every PTY chunk forever** (`pty.rs:50-74`): `from_utf8_lossy` +
  up to 7 `contains` passes per read, for the life of a loop session, though the TUI queries
  only occur at startup. Early-return unless the chunk contains `0x1b`; stop after startup.
- **L11 — Drag-over hit testing per pointermove** (`PaneGrid.tsx:70-92`): two
  `document.elementFromPoint` calls per move event (can exceed 60 Hz). rAF-coalesce.
- **L12 — Titlebar service dot animates `box-shadow` infinitely** (`styles/titlebar.css:212`):
  box-shadow can't composite — repaints every frame while any service runs. Pulse a
  `transform: scale()` + `opacity` ring instead.
- **L13 — Rail collapse animates `width`** (`styles/rail-collapse.css:3`): re-layouts the app
  body (and re-fits terminals) every frame for 180 ms. One-off interaction; transform-based
  slide would avoid it.
- **L14 — Loop Stop hook reads the transcript twice per firing** (`loophook.rs:257,283`):
  `extract_assistant_texts` + a second `read_to_string` of the same file for the sentinel check;
  O(iterations × transcript) × 2 over a long loop. Read once.
- **L15 — `ai_name_space` polls `try_wait` without draining pipes** (`ai.rs:61-73`): stderr
  bigger than the pipe buffer deadlocks the child into the full 30 s timeout, then falls back
  and spawns claude again. Drain on reader threads.
- **L16 — Minor**: `list_dir` recomputes `to_lowercase()` inside its sort comparator and
  `find_files` uses `p.is_dir()` (extra stat) over the free `e.file_type()` (`devtools/fs.rs:69,140`);
  duplicate `loadState("settings")` at boot (`AuthGate.tsx:47` + `App.tsx:167`).
- **Informational**: `tauri.conf.json` passes `--disable-background-timer-throttling
  --disable-renderer-backgrounding --disable-backgrounding-occluded-windows` — the webview runs
  at full tick while minimized. Presumably intentional for live terminals; flagged so it's a
  known trade, not an accident.

---

## Verified clean (checked, keep it this way)

- **PTY output hot path** (`pty.rs`): leading-edge flush for keystroke echo + 4 ms/16 KB
  batching under load; output crosses IPC as raw binary (`InvokeResponseBody::Raw`), received as
  a zero-copy `Uint8Array` view; bounded `sync_channel(256)` gives real backpressure within
  Rust; session drop (→ `ClosePseudoConsole`) happens off-thread; `kill_all` on exit prevents
  orphaned ConPTY hosts.
- **Pane lifecycle**: one flat grid holds all activated spaces' panes keyed by session id,
  hidden with `display:none` — space switches and cross-space moves never remount xterm; lazy
  activation avoids boot-time mass PTY spawn; `TerminalPane` is memoized with stable callbacks;
  WebGL attaches only for the active space (stays under the browser context cap) with correct
  loss handling; unmount cleanup tears down RAF, timers, observers, listeners, addons, buffers,
  and the PTY.
- **Resize handling**: rAF-synced ~60 ms throttled fit + 140 ms settle, 0-size guard, scroll
  preservation — no ResizeObserver feedback loop (remaining gap is only P10's IPC no-ops).
- **Chat engine**: 55 ms text-delta batching, watchdog/timers cleaned in `killLive`, superseded-
  process guard, persistence only at turn boundaries, disk blob capped.
- **Loops engine**: `setTimeout` re-armed after each iteration (no interval pileup), the 2^31
  overflow chaining in `armAt` is correct, exponential retry + FAIL_LIMIT, mandatory
  max-iterations, all in-memory buffers capped (logs 4000 / events 1500 / history 50).
- **Zustand hygiene**: no naked `useStore()` subscriptions anywhere; no selector returns a fresh
  object (the risky ones use stable constants/derived primitives) — no getSnapshot churn. The
  issues above are about *which* keys chrome subscribes to, not selector correctness.
- **Persistence discipline**: workspace saves debounced 300 ms + signature-compared + flushed on
  hide; settingsSync debounced 300 ms with remote-echo guard; `persist.rs` writes are atomic
  (temp + fsync + rename) with name sanitization.
- **Terminal search buffers** (`terminal/buffers.ts`): O(1) append, 400 ms deferred ANSI strip
  over the tail only, 24 KB/session hard cap — a well-done hot-path design.
- **Devtools commands**: git/worktree/project/fs/providers/mcp/skills/usage are all
  `async` + `spawn_blocking`, with sensible bounds (2 MB `read_file` cap, 100-hit/4000-dir
  `find_files`, 40-session cap, 30-day/160-file usage window).
- **CSS**: no `transition: all`, no universal selectors with filters/shadows, backdrop-filter
  only on small transient overlays, `prefers-reduced-motion` kill-switch, other infinite
  animations are transform/opacity and gated to transient states.
- **Startup**: hydration runs in effects after first render; `AuthGate` paints immediately;
  supabase `getSession` is a local read with a timeout guard; single-flight guards on the
  session tracker, auto-namers, and usage loads all present and correct.

## Suggested attack order

1. **P2 + P3 + P4 + P16** — one mechanical sweep: make every remaining sync command
   `async` + `spawn_blocking`, and fix the two kill/write-under-lock spots (P3, P9). Biggest
   jank win for the least risk.
2. **P5** — one-line debounce; stops per-keystroke disk flushes.
3. **P6 + P11** — batch loop-output appends and narrow the Rail/Titlebar/LoopsPage selectors;
   makes running loops stop taxing the whole UI.
4. **P1** — xterm flow control; the only unbounded path, worth doing properly.
5. **P7 + P15** — startup: lazy-load CodeMirror/Settings/dialogs, chunk vendors, woff2 fonts.
6. **P8 + P10** — poll hygiene + resize no-ops.
7. Lows opportunistically when touching those files.
