# HyprSpace — Security Audit

Date: 2026-06-19 · Scope: Rust backend (`src-tauri/src`), Tauri config/capabilities, React frontend (`src`), orchestrator, auth.
Method: three focused read-only audits (backend / frontend+orchestrator / config) cross-checked.

**Threat model.** Every `#[tauri::command]` is callable from the webview. The webview only ever loads our **local** bundled assets (no remote page is loaded into an IPC-privileged window — confirmed), and the LLM orchestrator **cannot** call `invoke()` directly (it only emits `hyprspace` JSON blocks that *our* code parses). So the realistic path to the powerful IPC surface is a **renderer XSS**, which is why CSP (S1) is the highest-leverage item even though no XSS vector exists today.

Disposition legend: **[FIXED]** done this pass · **[DOC]** documented, needs your decision/testing · **[CLEARED]** checked and not an issue.

---

## Cleared (checked, not vulnerable)

- **Markdown XSS** — `ChatPanel` uses `react-markdown` + `remark-gfm` with **no** `rehype-raw` and **no** `dangerouslySetInnerHTML`. Repo-wide grep for `dangerouslySetInnerHTML|rehype-raw|eval(|new Function|innerHTML` = 0 matches. Raw HTML in model/tool output is escaped. **Safe.**
- **Subscription compliance** — the chat spawns the user's already-logged-in `claude` CLI verbatim; no API key, no SDK, no token read for auth. `provider_status` reads `~/.claude.json` / `.credentials.json` / Codex `auth.json` JWT for **display-only** fields (email/plan) and never forwards them. The Google/Supabase sign-in is the app's *own* PKCE flow, separate from claude.ai. **Compliant — no violation.**
- **Orchestrator command injection** — spawned panes type the command into a *bare shell PTY* as keystrokes; the PTY is launched as `powershell`/`$SHELL` with `args:[]`. Provider builders (`claudeCmd`/`geminiCmd`/`codexCmd`/`WSL_CMD`) emit **constant** strings with no model interpolation. `provider` is whitelisted, `count` clamped 1–8. **Model output can't smuggle argv/flags.**
- **Orchestrator path escape** — `slug()` replaces every non-`[a-zA-Z0-9_-]` run with `-`, collapsing `..`, `/`, `\`, `:`; `joinPath(base, slug(name))` can't escape the projects base. `startsWith(base)` is redundant defense. **Confined.**
- **License verification** (`license.rs`) — Ed25519 verify against an embedded pubkey with revocation list and length checks, re-verified on load. Not spoofable without the private key. **Correct.**
- **`reveal_path`** (`devtools/fs.rs`) — `canonicalize` + `is_dir` + leading-`-` reject is a real guard against scheme/flag injection into `explorer`/`open`/`xdg-open`. **Good.**
- **No remote IPC / dangerous flags** — `withGlobalTauri`, `dangerousRemoteDomainIpcAccess`, `dangerousDisableAssetCspModification`, `devtools` overrides are all unset; `frontendDist` is local. **Good.**

---

## S1 — No Content-Security-Policy (`csp: null`)  ·  severity: HIGH  ·  [DOC]
`src-tauri/tauri.conf.json` (`app.security.csp = null`). The webview has no script/connect/object/frame restrictions while rendering LLM markdown and exposing a very powerful IPC surface (S2). Any future renderer XSS escalates straight to the IPC (PTY spawn, fs writes). **Not applied automatically** because a strict CSP (`script-src 'self'`) breaks Vite's dev HMR (inline scripts / eval / ws), and you're running in dev — applying it blind while you're away risks white-screening the app. **Recommendation:** add a production CSP and verify a `tauri build`:
```
default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline';
img-src 'self' data: https:; connect-src 'self' ipc: http://ipc.localhost https://*.supabase.co;
object-src 'none'; frame-src 'none'; base-uri 'self'
```
Tauri injects hashes for its own scripts when a CSP is set. Test login + Google avatar + chat after building.

## S2 — Powerful custom IPC commands reachable from the renderer  ·  severity: HIGH (by design)  ·  [DOC]
`create_pty` (arbitrary program/args/env/cwd — inherent to a terminal app), plus `git_*`, `create_project_dir`, `skill_write/delete`, `worktree_*`. Tauri capabilities do **not** gate custom commands, so a renderer compromise reaches all of them. The mitigation is S1 (CSP) + not loading remote content (already true), **not** removing the commands. Several inputs (`cwd`, project `path`, skill `cwd`) are legitimately user-driven absolute paths (folder picker, active workspace), so server-side path confinement would break real features — confinement is therefore intentionally **not** added except where the input is always a fixed token (see S5). Documented as accepted-with-mitigation.

## S3 — OAuth loopback: no `state`/path check, single read  ·  severity: MEDIUM  ·  [DOC]
`src-tauri/src/oauth.rs`. The `127.0.0.1:8765` listener returns the first request's target to the frontend with no `state`/path validation and a single 4 KB read. **Account takeover is prevented by PKCE** (Supabase validates the `code_verifier` the attacker can't have), so this is robustness + defense-in-depth, not a live takeover. Left untouched to avoid destabilizing working auth while unattended. **Recommendation:** loop the read until `\r\n\r\n`, ignore requests without `code=`, and add a `state` round-trip.

## S4 — Capabilities applied to both windows; `opener`/`create-webview` unscoped  ·  severity: MEDIUM  ·  [DOC]
`src-tauri/capabilities/default.json` grants the full set to `["main","settings"]`, including `core:webview:allow-create-webview-window` and unscoped `opener:allow-open-path`. Least-privilege: split per-window and scope `opener` to `https:` + project paths. **Not changed** (risk of breaking window/opener flows while unattended); see permissions inventory below.

## S5 — `persist` state `name` path traversal  ·  severity: MEDIUM  ·  [FIXED]
`src-tauri/src/persist.rs` joined a caller-supplied `name` directly (`{name}.json`); an absolute/`..` name escapes `~/.hyprspace/v2`. All real names are fixed tokens (`chat`, `settings`, …), so validating `name` to `[A-Za-z0-9_-]+` closes it with zero functional impact. **Fixed.**

## S6 — `cli_version` builds a `cmd /c "<cli> --version"` shell string  ·  severity: LOW (latent)  ·  [FIXED]
`src-tauri/src/devtools/providers.rs`. `cli` is whitelisted to `claude|gemini|codex` so not exploitable today, but it's an interpolated-shell footgun. **Fixed** by passing `cli` and `--version` as separate argv entries (no shell string).

## S7 — WebView2 args disable SmartScreen  ·  severity: MEDIUM  ·  [DOC]
`src-tauri/tauri.conf.json` `additionalBrowserArgs` includes `--disable-features=…,msSmartScreenProtection,…`. Removes reputation-based download/nav protection. **Recommendation:** drop `msSmartScreenProtection` from the disable list (keep the backgrounding flags). Left for you to verify it wasn't working around a specific bug.

## S8 — Operator actions auto-execute with no confirmation  ·  severity: MEDIUM  ·  [DOC]
`stores/chat.ts` → `runCommandsFor` on each `result` runs `hyprspace` blocks with no user gesture; the 1–8 `count` cap is **per block**, and blocks can chain, so a prompt-injected reply could create projects / spawn many agents — and spawned panes inherit the user's permission mode (possibly `bypass`). **Recommendation:** cap total spawned panes per turn, force chat-spawned agents off `bypass`, and/or render actions as click-to-run chips. Left as a product decision (changing it alters the feature you designed).

## Operational confirmations (no code change)
- **Updater** — HTTPS endpoint + minisign `pubkey` present, no insecure flag. Confirm the **private** signing key lives only in your offline/CI secret store and enable 2FA + tag protection on `hyprspace-releases`.
- **Supabase** — the committed key is the publishable/anon key (safe to ship) **only if RLS is default-deny on every table**. Verify RLS.

---

## Permissions inventory (`capabilities/default.json`, windows: main + settings)
- `core:default`, `window-state:default` — justified.
- `opener:default`, `opener:allow-open-path` — **unscoped** (S4); narrow to `https:` + project paths.
- `dialog:default` — justified (pickers).
- `clipboard-manager:allow-read-text` / `allow-write-text` — justified (copy/paste).
- `core:window:*` (minimize/maximize/close/hide/show/focus/center/position/drag/resize/state reads) — justified (custom titlebar, `decorations:false`).
- `core:window:allow-set-always-on-top` — droppable unless a UI exposes it.
- `core:webview:allow-create-webview-window` — **broad** (S4); only one window needs it.
- `core:event:*` (listen/unlisten/emit/emit-to) — justified (PTY/chat streaming).
- `updater:default` — justified (signed updater).
- `process:allow-restart` — justified; good that `allow-exit`/`allow-spawn` are absent.
- `core:app:allow-version` — justified.
- Not present (good): no `shell:*`, no `fs:*`, no `http:allow-fetch`. (Command exec is via the custom `create_pty`, not the shell plugin — see S2.)
