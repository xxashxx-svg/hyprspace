# HyprSpace audit — 2026-06-19

Full security + bug audit of the app (Rust backend, Tauri config/capabilities, React frontend, orchestrator, auth).

- [`security.md`](./security.md) — security findings (S1–S8) + permissions inventory + cleared items.
- [`bugs.md`](./bugs.md) — reliability/correctness bugs (B1–B10).

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
