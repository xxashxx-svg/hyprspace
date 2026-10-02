# Security policy

## Reporting a vulnerability

Report it privately through GitHub. Open a **draft security advisory** on this repo (Security tab,
then Advisories, then *Report a vulnerability*). Please don't open a public issue for anything
exploitable.

Include what someone would need to reproduce it: OS, app version, steps, and the impact. You'll get
a reply on the advisory thread. Please give us a reasonable window to ship a fix before you disclose
publicly.

## In scope

- The engine (`crates/engine`): PTY handling, the launch commands typed into shells, file reads,
  git operations, the persisted state store, and the loopback listener Claude's hooks report to.
- The harnesses (`crates/harness`): anything that lets agent output, file contents or repo names
  escape into a shell command or answer an approval on the user's behalf.
- The UI (`crates/ui`): terminal escape sequences, links and paths that open something the user
  didn't pick.
- The auto-update path (`crates/update`): manifest handling and signature verification.
- Credential handling: anything that leaks the CLIs' stored tokens to disk, logs or anywhere but the
  provider's own usage endpoint.

## Out of scope

- Vulnerabilities in the agent CLIs themselves (`claude`, `gemini`, `codex`). Report those upstream.
- The fact that a terminal pane can run arbitrary commands. That's the product.
- Missing hardening we're already aware of, unless you have a working exploit.
- Anything that requires an attacker who already has local code execution as the user.
