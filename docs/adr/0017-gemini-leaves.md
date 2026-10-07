# ADR 0017: Gemini leaves

- Status: Accepted
- Date: 2026-10-07
- Supersedes: "Gemini is an `Agent`" in ADR 0006

## Context

Gemini came over from the Tauri app as a third agent that only ran in a terminal: no harness, no
hooks, so no live state in the sidebar, no transcript and no usage limits. Ash: "remove gemini
its not even usable, t3 code, zeron no one has it". It cost a branch in every match on `Agent`,
an install line in the intro, a card in Settings and an `Agent::structured()` check in three
places.

## Decision

- `Agent` is Claude and Codex. `Agent::structured()` and the `Option` around
  `harness::for_agent` are gone, since every agent has a harness now.
- Its catalog, launch command, process detection, provider status, local usage reader, brand
  color and logo go with it. Settings' Activity still reads OpenCode and Grok.
- A state saved before this can name Gemini. `requests::without_gemini` rewrites it before it is
  parsed: a terminal thread that ran Gemini keeps its shell with no agent, and the composer's last
  agent and Gemini's model pick are dropped. Without it the whole file would fail to parse and the
  app would start clean.
- The Tauri importer skips Gemini panes, as it skips other kinds it can't carry over.

## Consequences

- A Gemini typed into a terminal by hand still runs; the sidebar just doesn't know it's an agent.
- Bringing Gemini back means a harness for it, not just the enum variant.
