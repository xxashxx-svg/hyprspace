# HyprSpace docs

Start with [AGENTS.md](../AGENTS.md), the project guide for people and agents alike.

Internal notes keep the decisions, constraints and traps the source alone doesn't explain. Most
changes need no doc change; follow the [documentation rules](../AGENTS.md#documentation) before
adding one.

- [Architecture](./internals/overview.md): the channel, the engine, saved state, GPUI
- [Glossary](./internals/glossary.md)
- [Structured sessions](./internals/sessions.md): the harnesses, permission modes, journals
- [Terminal sessions](./internals/terminals.md): PTYs, agent launches, hooks, the emulator
- [Threads, the sidebar and the main area](./internals/threads.md)
- [Dock, viewer and editor](./internals/viewer.md)
- [Usage and activity](./internals/usage.md)
- [Updates and installers](./internals/updates.md)
- [The phone](./internals/phone.md): the bridge, pairing, the board, terminal frames
- [Other computers](./internals/machines.md): threads that run on another computer you own

Runbooks:

- [Development](./operations/development.md): setup, the dev loop, the check, packages
- [Release](./operations/release.md): `deploy.ps1`, notes, which digit

[CHANGELOG.md](./CHANGELOG.md) holds the release notes; the app bundles it for What's new.
