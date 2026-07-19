# Security policy

## Reporting a vulnerability

Report privately through GitHub — open a **draft security advisory** on this repo
(Security tab → Advisories → *Report a vulnerability*). Please don't open a public issue for
anything exploitable.

Include what you'd need to reproduce it: OS, app version, steps, and impact. You'll get a reply
on the advisory thread. Please give us a reasonable window to ship a fix before disclosing.

## In scope

- The Tauri backend (`src-tauri/`) — PTY handling, the `#[tauri::command]` surface, file read/write,
  the persisted state store, license verification, the OAuth loopback listener.
- The frontend (`src/`) — anything that lets untrusted content (agent output, file contents, repo
  names) execute code, escape into a shell command, or reach the IPC bridge.
- Tauri capability grants (`src-tauri/capabilities/`) that are broader than they need to be.
- The auto-update path — manifest handling, signature verification.
- Credential handling: API keys in the OS keychain, anything that leaks tokens to disk or logs.

## Out of scope

- Vulnerabilities in the agent CLIs themselves (`claude`, `gemini`, `codex`) — report those upstream.
- The fact that a terminal pane can run arbitrary commands. That's the product.
- Missing hardening that's already tracked in [docs/audit/](./docs/audit/README.md), unless you have
  a working exploit.
- Anything requiring an attacker who already has local code execution as the user.
