# HyprSpace — Bug Audit

Date: 2026-06-19 · Companion to `security.md`. Reliability/correctness issues (non-security).

Disposition: **[FIXED]** done this pass · **[DOC]** documented / lower priority · **[CLEARED]** checked, fine.

---

## B1 — Mutex `.lock().unwrap()` panics on a poisoned lock  ·  severity: MEDIUM  ·  [FIXED]
`src-tauri/src/pty.rs`, `src-tauri/src/chat.rs`. Both managers hold `Mutex<HashMap<…>>` and `.lock().unwrap()`. If any thread panics while holding the lock, every later `lock().unwrap()` panics too, wedging PTY/chat I/O. `persist.rs` already recovers with `unwrap_or_else(|e| e.into_inner())`. **Fixed** — pty/chat now use the same poison-tolerant lock helper.

## B2 — `busy` can stick forever if a turn emits no `result`/`exit`  ·  severity: MEDIUM  ·  [FIXED]
`stores/chat.ts`. Send is gated on `!busy`; if `claude` wedges (no `result`, process alive → no `exit`), the chat locks until the user clicks Stop. **Fixed** with an inactivity watchdog: a timer reset on every event; after a long silence while busy it clears `busy` and surfaces a soft "timed out" note. Generous timeout so legitimate long agentic turns aren't interrupted.

## B3 — Persisted chat grows unbounded (messages + full tool results)  ·  severity: LOW  ·  [FIXED]
`stores/chat.ts`. Threads were capped at 30 but each thread's `messages` array was unbounded and stored full tool `result` strings; the whole blob is re-serialized every turn. **Fixed** — cap messages per thread and truncate stored tool results when persisting (UI already slices for display).

## B4 — `load()` trusts persisted shape → corrupt blob can white-screen the chat  ·  severity: LOW  ·  [FIXED]
`stores/chat.ts`. `load()` only checked `Array.isArray(threads)` then cast; a malformed `messages`/`blocks` shape throws in render. **Fixed** — normalize each thread on load (ensure arrays, drop malformed messages/blocks).

## B5 — `worktree_create/remove/list` are sync commands that block the UI thread  ·  severity: LOW  ·  [FIXED]
`src-tauri/src/devtools.rs`. These shell out to `git` synchronously on the IPC thread (`git worktree add` can take seconds → "Not Responding"), the exact issue other git commands were refactored to avoid. **Fixed** — made async + `spawn_blocking`, matching the rest.

## B6 — Chat `start` can orphan a process if `insert` overwrites a live one  ·  severity: LOW  ·  [FIXED]
`src-tauri/src/chat.rs`. Two `start`s for the same id could both spawn; the overwritten `Proc` was dropped without `taskkill`, orphaning a `cmd → claude` tree. **Fixed** — `start` now reaps any `Proc` it displaces on insert.

## B7 — OAuth loopback single 4 KB read can truncate the request  ·  severity: LOW  ·  [DOC]
`src-tauri/src/oauth.rs`. A fragmented TCP read could yield a partial request line and intermittently fail sign-in. Left untouched to avoid destabilizing working auth unattended; see `security.md` S3 for the recommended read-loop fix.

## B8 — Stale-turn routing (theoretical) when reusing the live process  ·  severity: LOW  ·  [CLEARED]
`stores/chat.ts`. Reuse mutates `live.asstId` in place, so in principle a late event from turn N could land on turn N+1. In practice turns are **serialized**: `result` is the last event of a turn and clears the throttle + sets `busy:false` before the next turn can start, so no turn-N events exist afterward. **Mitigated by serialization** — left as-is to avoid added complexity.

## B9 — `git_changes` porcelain parsing fragile for renamed/quoted paths  ·  severity: LOW  ·  [DOC]
`src-tauri/src/devtools.rs`. `split(" -> ")` + quote-trim mishandles paths containing `" -> "` or C-quoted escapes → wrong per-file line counts (cosmetic). Byte-slicing `line[..2]` is safe (the XY prefix is ASCII). **Recommendation:** switch to `--porcelain=v2 -z` if exact rename mapping ever matters. Not fixed (cosmetic).

## B10 — `taskkill /T /F` by PID is best-effort, no fallback  ·  severity: LOW  ·  [DOC]
`src-tauri/src/chat.rs`. Kills by PID without a `try_wait` first (tiny PID-reuse window) and ignores `taskkill` failure with no `child.kill()` fallback. Low likelihood. **Recommendation:** `try_wait()` before `taskkill`, fall back to `child.kill()`. Not fixed (low risk, current behavior works).
