# HyprSpace audit — 2026-06-19

Full security + bug audit of the app (Rust backend, Tauri config/capabilities, React frontend, orchestrator, auth).

- [`security.md`](./security.md) — security findings (S1–S8) + permissions inventory + cleared items.
- [`bugs.md`](./bugs.md) — reliability/correctness bugs (B1–B10).
- [`performance.md`](./performance.md) — performance audit 2026-07-15 (P1–P16 + lows), findings only, nothing fixed yet.

## Headline
The scary theoretical risks are **cleared**: no markdown XSS (no raw HTML), subscription compliance intact (no token ever used for auth), the orchestrator can't smuggle shell commands or escape the projects folder. The most valuable open item is **S1 (set a CSP)** — left for you to apply on a production build since a strict CSP breaks dev HMR.

## Fixed this pass (safe, non-breaking)
- **S5** persist `name` path-traversal → validated to a token.
- **S6** `cli_version` shell-string interpolation → args passed separately.
- **B1** pty/chat mutex poison → poison-tolerant lock.
- **B2** chat `busy` could stick forever → inactivity watchdog.
- **B3** persisted chat unbounded → cap messages + truncate stored tool results.
- **B4** corrupt persisted chat could white-screen → shape validation on load.
- **B5** worktree commands froze the UI → async + spawn_blocking.
- **B6** chat `start` could orphan a displaced process → reap on insert.

## Left for you (documented, needs a decision or prod testing)
- **S1** CSP (test on `tauri build`), **S3/B7** OAuth listener hardening (PKCE already protects), **S4** per-window capability scoping, **S7** re-enable SmartScreen, **S8** confirm/gate auto-run operator actions, plus operational checks (updater key custody, Supabase RLS). B9/B10 are cosmetic/low-risk.

## Follow-up audit — 2026-07-02 (post-0.9.0)
A second pass covered everything shipped since (usage dashboard, GPU terminals, editor, the Automations
overhaul). 25 findings, all fixed the same day:
- **HIGH:** interactive-automation stop double-finishing (duplicate history + bogus "finished" notification,
  could even kill a restarted run); `setTimeout` overflow made monthly+ crons fire immediately and repeatedly;
  the editor silently destroyed unsaved edits on every close path except the X button (now flushes to disk on
  unmount unless explicitly discarded); confirm-dialog Enter fired the destructive action even with Cancel focused.
- **MEDIUM:** delete-while-running ghost runs; cron time-budget measured from arming; codex resume dropping
  sandbox/model flags; pane mode ignoring schedules; diff summaries blaming pre-existing changes (now delta'd
  against a run-start baseline); unbounded codex rollout reads; UI-thread pipe writes (`chat_turn`/`write_pty`
  now async); `Store::backup()` path sanitization; usage-panel refresh races; plain terminals matching as
  "running services".
- Also closed from the original list: **S7** (SmartScreen flag removed), **B7** (read-until-headers), **B9**
  (`--porcelain -z` parsing), **B10** (try_wait + kill fallback in all three managers). Loop temp dirs now
  cleaned via `cleanup_hook_run`; dead commands (`provider_usage`, `prepare_hook_settings`) deregistered.
- **Still open by choice:** S1 (CSP — production-build test), S8 (orchestrator auto-exec gating — product
  decision), S3 state-param validation. Note for S2: keychain loop-API keys are now inside a renderer
  compromise's blast radius via `agent_start` secrets — CSP (S1) is the mitigation.
