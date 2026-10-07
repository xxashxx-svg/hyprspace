# Usage and activity

## Live limits

`engine/src/usage/live.rs` reads the token each CLI already stores and sends it to that provider's
own usage endpoint, as AGENTS.md's first rule allows.

- **The engine owns the floor.** A request inside 180 s (Claude) or 60 s (Codex) is answered from
  the last reading, and 429 or 5xx backs off, so no view can ask faster by mistake. Claude's bucket
  is shared with Claude Code itself.
- **The UI owns the cadence.** One `ui::usage::Limits` entity asks on a 30-second tick when each
  provider is due and holds every reading, so the ring in the title bar and Settings never disagree
  and opening Settings fetches nothing.
- **Free sources fill in.** The hook listener tees Claude's status line, and `usage/status.rs`
  turns its `rate_limits` into the same shape the endpoint gives; the freshest report wins and old
  ones are marked stale. Codex's session files are read only while its live reading has no windows.
- `HYPRSPACE_USAGE_FIXTURES` points the readers at files (a `fixtureStatus` fakes a 401 or 429), so
  the app can be checked without spending the bucket.

## Activity

Settings, Usage, Activity shows each agent's tokens and sessions from its own files
(`engine/src/usage/local.rs`), display only, after Claude Code's `/stats`.

- **Claude counts the way `/stats` does.** Claude writes one reply over several transcript lines,
  and each line repeats the reply's usage. `/stats` and `~/.claude/stats-cache.json` add every
  line, so they run about three times what the API billed (on 2026-10-06: 1.97B tokens for Opus 5.5
  in the stats file, 0.55B counting each reply once). We count the same way because Claude deletes
  transcripts after 30 days, so anything older exists only in the stats file's count. Counting
  replies once would be closer to the bill but leave a cliff at day 30, and wouldn't match what
  users compare it to.
- **The stats file wins where it reaches.** Every day through its `lastComputedDate` takes its
  totals and sessions from the file, which keeps days whose transcripts are gone. The transcripts
  cover the days after it. The file only updates when someone runs `/stats`.
- **Splits are estimated on old days.** The stats file has a total per model per day but no split.
  A day whose parts don't add up to its total splits it the way the model splits over all time
  (`modelUsage`), so over all time the split is exact and over 7 or 30 days it's an estimate.
- **Codex** counts each session's `total_token_usage` once, from a year of rollouts. Its
  `input_tokens` includes the cached part.
