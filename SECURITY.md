# Security policy

## Reporting a vulnerability

Report it privately through GitHub. Open a **draft security advisory** on this repo (Security tab,
then Advisories, then *Report a vulnerability*). Please don't open a public issue for anything
exploitable.

Include what someone would need to reproduce it: OS, app version, steps, and the impact. You'll get
a reply on the advisory thread. Please give us a reasonable window to ship a fix before you disclose
publicly.

## In scope

- The Tauri backend (`src-tauri/`): PTY handling, the `#[tauri::command]` surface, file read/write,
  the persisted state store, license verification, the OAuth loopback listener.
- The frontend (`src/`): anything that lets untrusted content such as agent output, file contents or
  repo names execute code, escape into a shell command, or reach the IPC bridge.
- Tauri capability grants (`src-tauri/capabilities/`) that are broader than they need to be.
- The auto-update path: manifest handling and signature verification.
- Credential handling: API keys in the OS keychain, and anything that leaks tokens to disk or logs.

## Out of scope

- Vulnerabilities in the agent CLIs themselves (`claude`, `gemini`, `codex`). Report those upstream.
- The fact that a terminal pane can run arbitrary commands. That's the product.
- Missing hardening we're already aware of (e.g. a not-yet-strict CSP), unless you have a working
  exploit.
- Anything that requires an attacker who already has local code execution as the user.
